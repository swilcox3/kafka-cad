use log::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

mod object_state {
    tonic::include_proto!("object_state");
}

mod submit {
    tonic::include_proto!("submit");
}

use submit::*;

struct SubmitService {}

#[tonic::async_trait]
impl submit_changes_server::SubmitChanges for SubmitService {
    async fn submit_changes(
        &self,
        request: Request<SubmitChangesInput>,
    ) -> Result<Response<SubmitChangesOutput>, Status> {
        unimplemented!();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let obj_url = std::env::var("OBJECTS_URL").unwrap();
    let dep_url = std::env::var("DEPENDENCIES_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();

    let svc = submit_changes_server::SubmitChangesServer::new(SubmitService {});

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
