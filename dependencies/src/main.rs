use tonic::transport::Server;
use tonic::{Request, Response, Status};
use trace_lib::*;
use tracing::*;
use tracing_futures::Instrument;

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
struct DepsService {
    redis_url: String,
}

#[tonic::async_trait]
impl dependencies_server::Dependencies for DepsService {
    #[instrument]
    async fn get_all_dependencies(
        &self,
        request: Request<GetAllDependenciesInput>,
    ) -> Result<Response<GetAllDependenciesOutput>, Status> {
        propagate_trace(request.metadata());
        let msg = request.get_ref();
        info!("Get all dependencies: {:?}", msg);
        let mut redis_conn = get_redis_conn(&self.redis_url).await?;
        let references = cache::get_all_deps(&mut redis_conn, &msg.file, msg.offset, &msg.ids)
            .instrument(info_span!("get_all_deps"))
            .await
            .map_err(to_status)?;
        Ok(Response::new(GetAllDependenciesOutput { references }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let jaeger_url = std::env::var("JAEGER_URL").unwrap();
    let redis_url = std::env::var("REDIS_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();
    let topic = std::env::var("TOPIC").unwrap();
    init_tracer(&jaeger_url, "dependencies")?;
    tokio::spawn(update_cache(redis_url.clone(), broker, group, topic));

    let svc = dependencies_server::DependenciesServer::new(DepsService { redis_url });

    println!("Running on {:?}", addr);
    Server::builder()
        .add_service(svc)
        .serve(addr)
        .await
        .unwrap();
    Ok(())
}
