use tracing::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use trace_lib::*;

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
        let span = info_span!("get_objects");
        propagate_trace(&span, request.metadata());
        let _enter = span.enter();
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
        let span = info_span!("get_latest_offset");
        propagate_trace(&span, request.metadata());
        let _enter = span.enter();
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
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let jaeger_url = std::env::var("JAEGER_URL").unwrap();
    let redis_url = std::env::var("REDIS_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();
    let topic = std::env::var("TOPIC").unwrap();
    trace_lib::init_tracer(&jaeger_url, "objects")?;
    println!("redis_url: {:?}", redis_url);
    let client = redis::Client::open(redis_url).unwrap();
    let now = std::time::SystemTime::now();
    while now.elapsed().unwrap() < std::time::Duration::from_secs(30) {
        println!("Checking redis");
        if let Ok((redis_conn, fut)) = client.get_multiplexed_async_connection().await {
            tokio::spawn(fut);
            let redis_clone = redis_conn.clone();
            tokio::spawn(update_cache(redis_clone, broker, group, topic));

            let svc = objects_server::ObjectsServer::new(ObjectService { redis_conn });

            println!("Running on {:?}", run_url);
            Server::builder()
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
