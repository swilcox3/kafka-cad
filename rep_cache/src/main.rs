use tonic::transport::Server;
use tonic::{Request, Response, Status};
use trace_lib::*;
use tracing::*;
use tracing_futures::Instrument;

mod cache;
mod kafka;
use cache::*;
use kafka::*;

mod representation {
    tonic::include_proto!("representation");
}

mod geom {
    tonic::include_proto!("geom");
}

mod rep_cache {
    tonic::include_proto!("rep_cache");
}
use rep_cache::*;

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
struct RepCacheService {
    redis_url: String,
}

#[tonic::async_trait]
impl rep_cache_server::RepCache for RepCacheService {
    #[instrument]
    async fn get_object_representations(
        &self,
        request: Request<GetObjectRepresentationsInput>,
    ) -> Result<Response<GetObjectRepresentationsOutput>, Status> {
        propagate_trace(request.metadata());
        let msg = request.get_ref();
        let mut redis_conn = get_redis_conn(&self.redis_url).await?;
        let mut reps = Vec::new();
        for id in &msg.obj_ids {
            let rep = cache::get_object_rep(&mut redis_conn, &msg.file, &id)
                .instrument(info_span!("get_object_rep"))
                .await
                .map_err(to_status)?;
            reps.push(rep);
        }
        Ok(Response::new(GetObjectRepresentationsOutput { reps }))
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
    trace_lib::init_tracer(&jaeger_url, "rep_cache")?;
    tokio::spawn(update_cache(redis_url.clone(), broker, group, topic));

    let svc = rep_cache_server::RepCacheServer::new(RepCacheService { redis_url });

    println!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
