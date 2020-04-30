use log::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use serde_json::json;

mod api {
    include!(concat!(env!("OUT_DIR"), "/api.rs"));
}
use api::*;

mod representation {
    include!(concat!(env!("OUT_DIR"), "/representation.rs"));
}

mod walls {
    include!(concat!(env!("OUT_DIR"), "/walls.rs"));
}

mod object_state {
    include!(concat!(env!("OUT_DIR"), "/object_state.rs"));
}

mod obj_defs {
    include!(concat!(env!("OUT_DIR"), "/obj_defs.rs"));
}

mod undo {
    include!(concat!(env!("OUT_DIR"), "/undo.rs"));
}

struct ApiService {}

#[tonic::async_trait]
impl api_server::Api for ApiService {
    async fn begin_undo_event(
        &self,
        request: Request<BeginUndoEventInput>,
    ) -> Result<Response<BeginUndoEventOutput>, Status> {
        unimplemented!();
    }

    async fn undo_latest(
        &self,
        request: Request<UndoLatestInput>,
    ) -> Result<Response<UndoLatestOutput>, Status> {
        unimplemented!();
    }

    async fn redo_latest(
        &self,
        request: Request<RedoLatestInput>,
    ) -> Result<Response<RedoLatestOutput>, Status> {
        unimplemented!();
    }

    async fn create_wall(
        &self,
        request: Request<CreateWallInput>,
    ) -> Result<Response<CreateWallOutput>, Status> {
        unimplemented!();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let svc = api_server::ApiServer::new(ApiService {});

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
