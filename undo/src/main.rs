use thiserror::Error;
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};
use trace_lib::propagate_trace;
use tracing::*;
use tracing_futures::Instrument;

mod cache;
mod invert;
mod kafka;
use cache::*;
use kafka::*;

mod geom {
    tonic::include_proto!("geom");
}

mod object_state {
    tonic::include_proto!("object_state");
}
use object_state::*;

mod undo {
    tonic::include_proto!("undo");
}
use undo::*;

mod objects {
    tonic::include_proto!("objects");
}
use objects::*;
type ObjClient = objects_client::ObjectsClient<Channel>;

#[derive(Debug, Error)]
pub enum UndoError {
    #[error("No events to undo for user {0} in file {1}")]
    NoUndoEvent(String, String),
    #[error("Obj {0} not found in undo event {1} in file {2}")]
    NoObjInUndoEvent(String, String, String),
    #[error("Prost encode error: {0}")]
    ProstEncodeError(#[from] prost::EncodeError),
    #[error("Prost decode error: {0}")]
    ProstDecodeError(#[from] prost::DecodeError),
    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
    #[error("Redis error: {0:?}")]
    DatabaseError(#[from] redis::RedisError),
}

impl Into<tonic::Status> for UndoError {
    fn into(self) -> tonic::Status {
        let msg = format!("{}", self);
        let code = match self {
            UndoError::DatabaseError(..)
            | UndoError::ProstEncodeError(..)
            | UndoError::BincodeError(..)
            | UndoError::ProstDecodeError(..) => tonic::Code::Internal,
            UndoError::NoUndoEvent(..) | UndoError::NoObjInUndoEvent(..) => tonic::Code::NotFound,
        };
        tonic::Status::new(code, msg)
    }
}

pub fn to_status<T: Into<UndoError>>(err: T) -> tonic::Status {
    let obj_error: UndoError = err.into();
    obj_error.into()
}

pub fn unavailable<T: std::fmt::Debug>(err: T) -> Status {
    Status::unavailable(format!("Unable to connect to service: {:?}", err))
}

#[instrument]
async fn get_redis_conn(url: &str) -> Result<redis::aio::MultiplexedConnection, tonic::Status> {
    let client =
        redis::Client::open(url).map_err(|e| tonic::Status::unavailable(format!("{:?}", e)))?;
    match client.get_multiplexed_async_connection().await {
        Ok((redis_conn, fut)) => {
            tokio::spawn(fut);
            Ok(redis_conn)
        }
        Err(e) => Err(tonic::Status::unavailable(format!("{:?}", e))),
    }
}

#[derive(Debug)]
struct UndoService {
    redis_url: String,
    obj_url: String,
}

#[tonic::async_trait]
impl undo_server::Undo for UndoService {
    #[instrument]
    async fn begin_undo_event(
        &self,
        request: Request<BeginUndoEventInput>,
    ) -> Result<Response<BeginUndoEventOutput>, Status> {
        propagate_trace(request.metadata());
        let msg = request.get_ref();
        let mut redis_conn = get_redis_conn(&self.redis_url).await?;
        cache::begin_undo_event(&mut redis_conn, &msg.file, &msg.user)
            .instrument(info_span!("cache::begin_undo_event"))
            .await
            .map_err(to_status)?;
        Ok(Response::new(BeginUndoEventOutput {}))
    }

    #[instrument]
    async fn undo_latest(
        &self,
        request: Request<UndoLatestInput>,
    ) -> Result<Response<UndoLatestOutput>, Status> {
        propagate_trace(request.metadata());
        let msg = request.get_ref();
        let mut redis_conn = get_redis_conn(&self.redis_url).await?;
        let mut obj_client = objects_client::ObjectsClient::connect(self.obj_url.clone())
            .instrument(info_span!("objects_client::connect"))
            .await
            .map_err(unavailable)?;
        let (event, latest) = cache::undo(&mut redis_conn, &msg.file, &msg.user)
            .instrument(info_span!("cache::undo"))
            .await
            .map_err(to_status)?;
        let changes = invert::invert_changes(
            &mut obj_client,
            &msg.file,
            &msg.user,
            change_msg::ChangeSource::Undo(event),
            latest,
        )
        .instrument(info_span!("invert_changes"))
        .await?;
        Ok(Response::new(UndoLatestOutput { changes }))
    }

    #[instrument]
    async fn redo_latest(
        &self,
        request: Request<RedoLatestInput>,
    ) -> Result<Response<RedoLatestOutput>, Status> {
        propagate_trace(request.metadata());
        let msg = request.get_ref();
        let mut redis_conn = get_redis_conn(&self.redis_url).await?;
        let mut obj_client = objects_client::ObjectsClient::connect(self.obj_url.clone())
            .instrument(info_span!("objects_client::connect"))
            .await
            .map_err(unavailable)?;
        let (event, latest) = cache::redo(&mut redis_conn, &msg.file, &msg.user)
            .instrument(info_span!("redo"))
            .await
            .map_err(to_status)?;
        let changes = invert::invert_changes(
            &mut obj_client,
            &msg.file,
            &msg.user,
            change_msg::ChangeSource::Redo(event),
            latest,
        )
        .instrument(info_span!("invert_changes"))
        .await?;
        Ok(Response::new(RedoLatestOutput { changes }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let jaeger_url = std::env::var("JAEGER_URL").unwrap();
    let redis_url = std::env::var("REDIS_URL").unwrap();
    let obj_url = std::env::var("OBJECTS_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();
    let topic = std::env::var("TOPIC").unwrap();
    trace_lib::init_tracer(&jaeger_url, "undo")?;
    tokio::spawn(update_cache(
        redis_url.clone(),
        broker.clone(),
        group.clone(),
        topic.clone(),
    ));
    let svc = undo_server::UndoServer::new(UndoService { redis_url, obj_url });

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
