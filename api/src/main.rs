use tonic::transport::Server;
use tonic::{Request, Response, Status};
use trace_lib::TracedRequest;
use tracing::*;

mod api {
    tonic::include_proto!("api");
}
use api::*;

mod representation {
    tonic::include_proto!("representation");
}

mod geom {
    tonic::include_proto!("geom");
}

mod object_state {
    tonic::include_proto!("object_state");
}

mod operations {
    tonic::include_proto!("operations");
}

mod undo {
    tonic::include_proto!("undo");
}

mod submit {
    tonic::include_proto!("submit");
}

fn unavailable<T: std::fmt::Debug>(err: T) -> Status {
    Status::unavailable(format!("Couldn't connect to child service: {:?}", err))
}

#[derive(Debug, Clone)]
struct Prefix {
    file: String,
    user: String,
    offset: i64,
}

impl Prefix {
    pub fn new(prefix_opt: Option<OpPrefixMsg>) -> Result<Prefix, Status> {
        if let Some(prefix) = prefix_opt {
            Ok(Prefix {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
            })
        } else {
            Err(Status::invalid_argument("Operation prefix is required"))
        }
    }
}

struct ApiService {
    undo_url: String,
    ops_url: String,
    submit_url: String,
}

#[tonic::async_trait]
impl api_server::Api for ApiService {
    async fn begin_undo_event(
        &self,
        request: Request<BeginUndoEventInput>,
    ) -> Result<Response<BeginUndoEventOutput>, Status> {
        let msg = request.into_inner();
        info!("Begin Undo Event: {:?}", msg);
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .await
            .map_err(unavailable)?;
        let req = TracedRequest::new(
            undo::BeginUndoEventInput {
                file: msg.file,
                user: msg.user,
            },
            "api",
            "begin_undo_event",
        );
        undo_client.begin_undo_event(req).await?;
        Ok(Response::new(BeginUndoEventOutput {}))
    }

    async fn undo_latest(
        &self,
        request: Request<UndoLatestInput>,
    ) -> Result<Response<UndoLatestOutput>, Status> {
        let msg = request.into_inner();
        info!("Undo Latest: {:?}", msg);
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .await
            .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .await
                .map_err(unavailable)?;
        let prefix = Prefix::new(msg.prefix)?;
        let changes = undo_client
            .undo_latest(Request::new(undo::UndoLatestInput {
                file: prefix.file.clone(),
                user: prefix.user.clone(),
            }))
            .await?
            .into_inner();
        let mut output = submit_client
            .submit_changes(Request::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes: changes.changes,
            }))
            .await?
            .into_inner();
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(UndoLatestOutput { offset })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }

    async fn redo_latest(
        &self,
        request: Request<RedoLatestInput>,
    ) -> Result<Response<RedoLatestOutput>, Status> {
        let msg = request.into_inner();
        info!("Redo Latest: {:?}", msg);
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .await
            .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .await
                .map_err(unavailable)?;
        let prefix = Prefix::new(msg.prefix)?;
        let changes = undo_client
            .redo_latest(Request::new(undo::RedoLatestInput {
                file: prefix.file.clone(),
                user: prefix.user.clone(),
            }))
            .await?
            .into_inner();
        let mut output = submit_client
            .submit_changes(Request::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes: changes.changes,
            }))
            .await?
            .into_inner();
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(RedoLatestOutput { offset })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }

    async fn create_walls(
        &self,
        request: Request<CreateWallsInput>,
    ) -> Result<Response<CreateWallsOutput>, Status> {
        let msg = request.into_inner();
        info!("Create Walls: {:?}", msg);
        let mut ops_client =
            operations::operations_client::OperationsClient::connect(self.ops_url.clone())
                .await
                .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .await
                .map_err(unavailable)?;
        let prefix = Prefix::new(msg.prefix)?;
        let mut walls = Vec::new();
        for wall in msg.walls {
            walls.push(operations::WallMsg {
                first_pt: wall.first_pt,
                second_pt: wall.second_pt,
                width: wall.width,
                height: wall.height,
            });
        }
        let objects = ops_client
            .create_walls(Request::new(operations::CreateWallsInput { walls }))
            .await?
            .into_inner();
        let mut changes = Vec::new();
        let mut ids = Vec::new();
        for obj in objects.walls.into_iter() {
            ids.push(obj.id.clone());
            changes.push(object_state::ChangeMsg {
                user: prefix.user.clone(),
                change_type: Some(object_state::change_msg::ChangeType::Add(obj)),
            });
        }

        let mut output = submit_client
            .submit_changes(Request::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes,
            }))
            .await?
            .into_inner();
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(CreateWallsOutput {
                obj_ids: ids,
                offset,
            })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let jaeger_url = std::env::var("JAEGER_URL").unwrap();
    let undo_url = std::env::var("UNDO_URL").unwrap().parse().unwrap();
    let ops_url = std::env::var("OPS_URL").unwrap().parse().unwrap();
    let submit_url = std::env::var("SUBMIT_URL").unwrap().parse().unwrap();
    trace_lib::init_tracer(&jaeger_url, "api")?;
    let svc = api_server::ApiServer::new(ApiService {
        undo_url,
        ops_url,
        submit_url,
    });
    info!("Running on {:?}", run_url);
    Server::builder()
        .trace_fn(|_| tracing::info_span!("api"))
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
