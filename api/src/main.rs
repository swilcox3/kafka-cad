use tonic::transport::Server;
use tonic::{Request, Response, Status};
use trace_lib::{trace_response, TracedRequest};
use tracing::*;
use tracing_futures::Instrument;

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

#[derive(Debug)]
struct ApiService {
    undo_url: String,
    ops_url: String,
    submit_url: String,
}

#[tonic::async_trait]
impl api_server::Api for ApiService {
    #[instrument]
    async fn begin_undo_event(
        &self,
        request: Request<BeginUndoEventInput>,
    ) -> Result<Response<BeginUndoEventOutput>, Status> {
        let msg = request.into_inner();
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .instrument(info_span!("undo_client::connect"))
            .await
            .map_err(unavailable)?;
        let req = TracedRequest::new(undo::BeginUndoEventInput {
            file: msg.file,
            user: msg.user,
        });
        let resp = undo_client
            .begin_undo_event(req)
            .instrument(info_span!("undo_client::begin_undo_event"))
            .await;
        trace_response(resp)?;
        Ok(Response::new(BeginUndoEventOutput {}))
    }

    #[instrument]
    async fn undo_latest(
        &self,
        request: Request<UndoLatestInput>,
    ) -> Result<Response<UndoLatestOutput>, Status> {
        let msg = request.into_inner();
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .instrument(info_span!("undo_client::connect"))
            .await
            .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .instrument(info_span!("submit::connect"))
                .await
                .map_err(unavailable)?;
        let prefix = Prefix::new(msg.prefix)?;
        let req = TracedRequest::new(undo::UndoLatestInput {
            file: prefix.file.clone(),
            user: prefix.user.clone(),
        });
        let resp = undo_client
            .undo_latest(req)
            .instrument(info_span!("undo_latest"))
            .await;
        let changes = trace_response(resp)?;
        let resp = submit_client
            .submit_changes(Request::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes: changes.changes,
            }))
            .instrument(info_span!("submit_changes"))
            .await;
        let mut output = trace_response(resp)?;
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(UndoLatestOutput { offset })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }

    #[instrument]
    async fn redo_latest(
        &self,
        request: Request<RedoLatestInput>,
    ) -> Result<Response<RedoLatestOutput>, Status> {
        let msg = request.into_inner();
        let mut undo_client = undo::undo_client::UndoClient::connect(self.undo_url.clone())
            .instrument(info_span!("undo_client::connect"))
            .await
            .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .instrument(info_span!("submit_client::connect"))
                .await
                .map_err(unavailable)?;
        let prefix = Prefix::new(msg.prefix)?;
        let resp = undo_client
            .redo_latest(Request::new(undo::RedoLatestInput {
                file: prefix.file.clone(),
                user: prefix.user.clone(),
            }))
            .instrument(info_span!("redo_latest"))
            .await;
        let changes = trace_response(resp)?;
        let req = TracedRequest::new(submit::SubmitChangesInput {
            file: prefix.file,
            user: prefix.user,
            offset: prefix.offset,
            changes: changes.changes,
        });
        let resp = submit_client
            .submit_changes(req)
            .instrument(info_span!("submit_changes"))
            .await;
        let mut output = trace_response(resp)?;
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(RedoLatestOutput { offset })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }

    #[instrument]
    async fn create_walls(
        &self,
        request: Request<CreateWallsInput>,
    ) -> Result<Response<CreateWallsOutput>, Status> {
        let msg = request.into_inner();
        let mut ops_client =
            operations::operations_client::OperationsClient::connect(self.ops_url.clone())
                .instrument(info_span!("ops_client::connect"))
                .await
                .map_err(unavailable)?;
        let mut submit_client =
            submit::submit_changes_client::SubmitChangesClient::connect(self.submit_url.clone())
                .instrument(info_span!("submit_client::connect"))
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
        let resp = ops_client
            .create_walls(TracedRequest::new(operations::CreateWallsInput { walls }))
            .instrument(info_span!("create_walls"))
            .await;
        let objects = trace_response(resp)?;
        let mut changes = Vec::new();
        let mut ids = Vec::new();
        for obj in objects.walls.into_iter() {
            ids.push(obj.id.clone());
            changes.push(object_state::ChangeMsg {
                user: prefix.user.clone(),
                change_type: Some(object_state::change_msg::ChangeType::Add(obj)),
            });
        }

        let resp = submit_client
            .submit_changes(TracedRequest::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes,
            }))
            .instrument(info_span!("submit_changes"))
            .await;
        let mut output = trace_response(resp)?;
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
    println!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
