use log::*;
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};

mod object_state {
    tonic::include_proto!("object_state");
}

mod objects {
    tonic::include_proto!("objects");
}

mod dependencies {
    tonic::include_proto!("dependencies");
}

mod submit {
    tonic::include_proto!("submit");
}

use dependencies::*;
use object_state::*;
use objects::*;
use submit::*;

pub type ObjClient = objects_client::ObjectsClient<Channel>;
pub type DepClient = dependencies_client::DependenciesClient<Channel>;

mod produce;
mod update;

struct SubmitService {
    broker: String,
    topic: String,
    obj_client: ObjClient,
    dep_client: DepClient,
}

#[tonic::async_trait]
impl submit_changes_server::SubmitChanges for SubmitService {
    async fn submit_changes(
        &self,
        request: Request<SubmitChangesInput>,
    ) -> Result<Response<SubmitChangesOutput>, Status> {
        let msg = request.into_inner();
        info!("Submitting changes: {:?}", msg);
        let mut obj_client = self.obj_client.clone();
        let mut dep_client = self.dep_client.clone();
        let updated = update::update_changes(
            &mut obj_client,
            &mut dep_client,
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
    let broker = std::env::var("BROKER").unwrap();
    let topic = std::env::var("TOPIC").unwrap();
    let now = std::time::SystemTime::now();
    while now.elapsed().unwrap() < std::time::Duration::from_secs(30) {
        if let Ok(obj_client) = objects_client::ObjectsClient::connect(obj_url.clone()).await {
            while now.elapsed().unwrap() < std::time::Duration::from_secs(30) {
                if let Ok(dep_client) =
                    dependencies_client::DependenciesClient::connect(dep_url.clone()).await
                {
                    let svc = submit_changes_server::SubmitChangesServer::new(SubmitService {
                        broker,
                        topic,
                        obj_client,
                        dep_client,
                    });

                    info!("Running on {:?}", run_url);
                    Server::builder()
                        .add_service(svc)
                        .serve(run_url)
                        .await
                        .unwrap();
                    return Ok(());
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    panic!("Couldn't connect to dependencies or objects");
}
