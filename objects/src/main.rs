use tonic::transport::Server;
use tonic::{Request, Response, Status};
use trace_lib::*;
use tracing::*;
use tracing_futures::Instrument;

mod cache;
mod kafka;
use cache::*;
use kafka::*;

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
struct ObjectService {
    redis_url: String,
}

#[tonic::async_trait]
impl objects_server::Objects for ObjectService {
    #[instrument]
    async fn get_objects(
        &self,
        request: Request<GetObjectsInput>,
    ) -> Result<Response<GetObjectsOutput>, Status> {
        propagate_trace(request.metadata());
        let msg = request.get_ref();
        info!("Get objects: {:?}", msg);
        let mut redis_conn = get_redis_conn(&self.redis_url).await?;
        let objects = cache::get_objects(&mut redis_conn, msg)
            .instrument(info_span!("cache::get_objects"))
            .await
            .map_err(to_status)?;
        Ok(Response::new(GetObjectsOutput { objects }))
    }

    #[instrument]
    async fn get_latest_offset(
        &self,
        request: Request<GetLatestOffsetInput>,
    ) -> Result<Response<GetLatestOffsetOutput>, Status> {
        propagate_trace(request.metadata());
        let msg = request.get_ref();
        info!("Get latest offset: {:?}", msg);
        let mut redis_conn = get_redis_conn(&self.redis_url).await?;
        let offset = cache::get_latest_offset(&mut redis_conn, msg)
            .instrument(info_span!("cache::get_latest_offset"))
            .await
            .map_err(to_status)?;
        Ok(Response::new(GetLatestOffsetOutput { offset }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let jaeger_url = std::env::var("JAEGER_URL").unwrap();
    let redis_url = std::env::var("REDIS_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();
    let topic = std::env::var("TOPIC").unwrap();
    trace_lib::init_tracer(&jaeger_url, "objects")?;
    tokio::spawn(update_cache(redis_url.clone(), broker, group, topic));

    let svc = objects_server::ObjectsServer::new(ObjectService { redis_url });

    println!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
