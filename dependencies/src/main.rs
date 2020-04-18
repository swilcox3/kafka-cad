use log::*;
use redis::aio::MultiplexedConnection;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

mod cache;
mod kafka;
#[cfg(test)]
mod tests;
use cache::*;
use kafka::*;

mod dependencies {
    tonic::include_proto!("dependencies");
}

use dependencies::*;

struct DepsService {
    redis_conn: MultiplexedConnection,
}

#[tonic::async_trait]
impl dependencies_server::Dependencies for DepsService {
    async fn get_all_dependencies(
        &self,
        request: Request<GetAllDependenciesInput>,
    ) -> Result<Response<GetAllDependenciesOutput>, Status> {
        let msg = request.get_ref();
        info!("Get all dependencies: {:?}", msg);
        let mut redis_conn = self.redis_conn.clone();
        let references = cache::get_all_deps(&mut redis_conn, &msg.file, msg.offset, &msg.ids)
            .await
            .map_err(to_status)?;
        Ok(Response::new(GetAllDependenciesOutput { references }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let addr = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let redis_url = std::env::var("REDIS_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();
    info!("redis_url: {:?}", redis_url);
    let client = redis::Client::open(redis_url).unwrap();
    let (redis_conn, fut) = client.get_multiplexed_async_connection().await.unwrap();
    tokio::spawn(fut);
    let redis_clone = redis_conn.clone();
    tokio::spawn(update_cache(redis_clone, broker, group));

    let svc = dependencies_server::DependenciesServer::new(DepsService { redis_conn });

    info!("Running on {:?}", addr);
    Server::builder()
        .add_service(svc)
        .serve(addr)
        .await
        .unwrap();
    Ok(())
}
