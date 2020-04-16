use log::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

mod cache;
mod kafka;
use cache::*;
use kafka::*;

struct ObjectService {
    redis_conn: redis::aio::MultiplexedConnection,
}

#[tonic::async_trait]
impl objects_server::Objects for ObjectService {
    async fn get_objects(
        &self,
        request: Request<GetObjectsInput>,
    ) -> Result<Response<GetObjectsOutput>, Status> {
        let msg = request.get_ref();
        info!("Get objects: {:?}", msg);
        let mut redis_conn = self.redis_conn.clone();
        let objects = cache::get_objects(&mut redis_conn, msg)
            .await
            .map_err(to_status)?;
        Ok(Response::new(GetObjectsOutput { objects }))
    }

    async fn get_latest_offset(
        &self,
        request: Request<GetLatestOffsetInput>,
    ) -> Result<Response<GetLatestOffsetOutput>, Status> {
        let msg = request.get_ref();
        info!("Get latest offset: {:?}", msg);
        let mut redis_conn = self.redis_conn.clone();
        let offset = cache::get_latest_offset(&mut redis_conn, msg)
            .await
            .map_err(to_status)?;
        Ok(Response::new(GetLatestOffsetOutput { offset }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let redis_url = std::env::var("REDIS_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();
    let max_len: u64 = std::env::var("CACHE_LENGTH")
        .unwrap()
        .parse()
        .expect("CACHE_LENGTH must be a positive integer");
    info!("redis_url: {:?}", redis_url);
    let client = redis::Client::open(redis_url).unwrap();
    let (redis_conn, fut) = client.get_multiplexed_async_connection().await.unwrap();
    tokio::spawn(fut);
    let redis_clone = redis_conn.clone();
    tokio::spawn(update_cache(redis_clone, broker, group, max_len));

    let svc = objects_server::ObjectsServer::new(ObjectService { redis_conn });

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
