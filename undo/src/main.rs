use thiserror::Error;
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};
use trace_lib::propagate_trace;
use tracing::*;

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
            UndoError::NoUndoEvent(..) => tonic::Code::NotFound,
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

struct UndoService {
    redis_conn: redis::aio::MultiplexedConnection,
    obj_url: String,
}

#[tonic::async_trait]
impl undo_server::Undo for UndoService {
    async fn begin_undo_event(
        &self,
        request: Request<BeginUndoEventInput>,
    ) -> Result<Response<BeginUndoEventOutput>, Status> {
        let _span = propagate_trace(&request, "undo", "begin_undo_event");
        let msg = request.get_ref();
        let mut redis_conn = self.redis_conn.clone();
        cache::begin_undo_event(&mut redis_conn, &msg.file, &msg.user)
            .await
            .map_err(to_status)?;
        Ok(Response::new(BeginUndoEventOutput {}))
    }

    async fn undo_latest(
        &self,
        request: Request<UndoLatestInput>,
    ) -> Result<Response<UndoLatestOutput>, Status> {
        let _span = propagate_trace(&request, "undo", "undo_latest");
        let msg = request.get_ref();
        let mut redis_conn = self.redis_conn.clone();
        let mut obj_client = objects_client::ObjectsClient::connect(self.obj_url.clone())
            .await
            .map_err(unavailable)?;
        let latest = cache::undo(&mut redis_conn, &msg.file, &msg.user)
            .await
            .map_err(to_status)?;
        match invert::invert_changes(&mut obj_client, &msg.file, &msg.user, latest).await {
            Ok(changes) => Ok(Response::new(UndoLatestOutput { changes })),
            Err(e) => {
                cache::redo(&mut redis_conn, &msg.file, &msg.user)
                    .await
                    .map_err(to_status)?;
                Err(e)
            }
        }
    }

    async fn redo_latest(
        &self,
        request: Request<RedoLatestInput>,
    ) -> Result<Response<RedoLatestOutput>, Status> {
        let _span = propagate_trace(&request, "undo", "redo_latest");
        let msg = request.get_ref();
        let mut redis_conn = self.redis_conn.clone();
        let mut obj_client = objects_client::ObjectsClient::connect(self.obj_url.clone())
            .await
            .map_err(unavailable)?;
        let latest = cache::redo(&mut redis_conn, &msg.file, &msg.user)
            .await
            .map_err(to_status)?;
        match invert::invert_changes(&mut obj_client, &msg.file, &msg.user, latest).await {
            Ok(changes) => Ok(Response::new(RedoLatestOutput { changes })),
            Err(e) => {
                cache::undo(&mut redis_conn, &msg.file, &msg.user)
                    .await
                    .map_err(to_status)?;
                Err(e)
            }
        }
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
    info!("redis_url: {:?}", redis_url);
    let client = redis::Client::open(redis_url).unwrap();
    let now = std::time::SystemTime::now();
    while now.elapsed().unwrap() < std::time::Duration::from_secs(30) {
        info!("Checking redis");
        if let Ok((redis_conn, fut)) = client.get_multiplexed_async_connection().await {
            tokio::spawn(fut);
            let redis_clone = redis_conn.clone();
            tokio::spawn(update_cache(
                redis_clone,
                broker.clone(),
                group.clone(),
                topic.clone(),
            ));
            let svc = undo_server::UndoServer::new(UndoService {
                redis_conn,
                obj_url,
            });

            info!("Running on {:?}", run_url);
            Server::builder()
                .trace_fn(|_| tracing::info_span!("undo"))
                .add_service(svc)
                .serve(run_url)
                .await
                .unwrap();
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    panic!("Couldn't connect to redis");
}
