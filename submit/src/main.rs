use log::*;
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};

mod object_state {
    tonic::include_proto!("object_state");
}
use object_state::*;

mod geom {
    tonic::include_proto!("geom");
}
use geom::*;

mod objects {
    tonic::include_proto!("objects");
}

mod dependencies {
    tonic::include_proto!("dependencies");
}

mod representation {
    tonic::include_proto!("representation");
}

mod submit {
    tonic::include_proto!("submit");
}

mod operations {
    tonic::include_proto!("operations");
}

use dependencies::*;
use objects::*;
use operations::*;
use submit::*;

pub type ObjClient = objects_client::ObjectsClient<Channel>;
pub type DepClient = dependencies_client::DependenciesClient<Channel>;
pub type OpsClient = operations_client::OperationsClient<Channel>;

mod produce;
mod update;

fn to_status<T: std::fmt::Debug>(err: T) -> Status {
    Status::unavailable(format!("Couldn't connect to service: {:?}", err))
}

struct SubmitService {
    broker: String,
    topic: String,
    obj_url: String,
    dep_url: String,
    ops_url: String,
}

#[tonic::async_trait]
impl submit_changes_server::SubmitChanges for SubmitService {
    async fn submit_changes(
        &self,
        request: Request<SubmitChangesInput>,
    ) -> Result<Response<SubmitChangesOutput>, Status> {
        let msg = request.into_inner();
        info!("Submitting changes: {:?}", msg);
        let mut obj_client = objects_client::ObjectsClient::connect(self.obj_url.clone())
            .await
            .map_err(to_status)?;
        let mut dep_client = dependencies_client::DependenciesClient::connect(self.dep_url.clone())
            .await
            .map_err(to_status)?;
        let mut ops_client = operations_client::OperationsClient::connect(self.ops_url.clone())
            .await
            .map_err(to_status)?;
        let updated = update::update_changes(
            &mut obj_client,
            &mut dep_client,
            &mut ops_client,
            msg.file,
            msg.user,
            msg.offset,
            msg.changes,
        )
        .await?;
        let offsets = produce::submit_changes(&self.broker, &self.topic, updated).await;
        Ok(Response::new(SubmitChangesOutput { offsets }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let obj_url = std::env::var("OBJECTS_URL").unwrap();
    let dep_url = std::env::var("DEPENDENCIES_URL").unwrap();
    let ops_url = std::env::var("OPERATIONS_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let topic = std::env::var("TOPIC").unwrap();
    let svc = submit_changes_server::SubmitChangesServer::new(SubmitService {
        broker,
        topic,
        obj_url,
        dep_url,
        ops_url,
    });

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    return Ok(());
}
