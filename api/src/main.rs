use futures::StreamExt;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use trace_lib::{trace_response, TracedRequest};
use tracing::*;
use tracing_futures::Instrument;

mod common;

mod api {
    tonic::include_proto!("api");
}
use api::*;

mod representation {
    tonic::include_proto!("representation");
}

mod rep_cache {
    tonic::include_proto!("rep_cache");
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

mod objects {
    tonic::include_proto!("objects");
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
    obj_url: String,
    ops_url: String,
    submit_url: String,
    rep_cache_url: String,
}

#[tonic::async_trait]
impl api_server::Api for ApiService {
    #[instrument]
    async fn begin_undo_event(
        &self,
        request: Request<BeginUndoEventInput>,
    ) -> Result<Response<BeginUndoEventOutput>, Status> {
        let msg = request.into_inner();
        let mut undo_client = common::undo_client(self.undo_url.clone()).await?;
        let req = TracedRequest::new(undo::BeginUndoEventInput {
            file: msg.file,
            user: msg.user,
        });
        let resp = undo_client.begin_undo_event(req).await;
        trace_response(resp)?;
        Ok(Response::new(BeginUndoEventOutput {}))
    }

    #[instrument]
    async fn undo_latest(
        &self,
        request: Request<UndoLatestInput>,
    ) -> Result<Response<UndoLatestOutput>, Status> {
        let msg = request.into_inner();
        let mut undo_client = common::undo_client(self.undo_url.clone()).await?;
        let mut submit_client = common::submit_client(self.submit_url.clone()).await?;
        let prefix = Prefix::new(msg.prefix)?;
        let req = TracedRequest::new(undo::UndoLatestInput {
            file: prefix.file.clone(),
            user: prefix.user.clone(),
        });
        let resp = undo_client.undo_latest(req).await;
        let changes = trace_response(resp)?;
        let resp = submit_client
            .submit_changes(TracedRequest::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes: changes.changes,
            }))
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
        let mut undo_client = common::undo_client(self.undo_url.clone()).await?;
        let mut submit_client = common::submit_client(self.submit_url.clone()).await?;
        let prefix = Prefix::new(msg.prefix)?;
        let resp = undo_client
            .redo_latest(TracedRequest::new(undo::RedoLatestInput {
                file: prefix.file.clone(),
                user: prefix.user.clone(),
            }))
            .await;
        let changes = trace_response(resp)?;
        let req = TracedRequest::new(submit::SubmitChangesInput {
            file: prefix.file,
            user: prefix.user,
            offset: prefix.offset,
            changes: changes.changes,
        });
        let resp = submit_client.submit_changes(req).await;
        let mut output = trace_response(resp)?;
        match output.offsets.pop() {
            Some(offset) => Ok(Response::new(RedoLatestOutput { offset })),
            None => Err(Status::out_of_range(
                "No offsets received from submit service",
            )),
        }
    }

    type OpenFileStream = tokio::sync::mpsc::Receiver<Result<OpenFileOutput, Status>>;

    #[instrument]
    async fn open_file(
        &self,
        request: Request<OpenFileInput>,
    ) -> Result<Response<Self::OpenFileStream>, Status> {
        let msg = request.into_inner();
        let mut rep_cache_client = common::rep_cache_client(self.rep_cache_url.clone()).await?;
        let mut obj_client = common::objects_client(self.obj_url.clone()).await?;
        let resp = obj_client
            .get_latest_object_list(TracedRequest::new(objects::GetLatestObjectListInput {
                file: msg.file.clone(),
            }))
            .await;
        let mut stream = trace_response(resp)?;
        let (mut tx, rx) = tokio::sync::mpsc::channel(100);
        tokio::spawn(async move {
            while let Some(obj_id_res) = stream.next().await {
                match obj_id_res {
                    Ok(obj_id) => {
                        let input = rep_cache::GetObjectRepresentationsInput {
                            file: msg.file.clone(),
                            obj_ids: vec![obj_id.obj_id],
                        };
                        let resp = rep_cache_client
                            .get_object_representations(TracedRequest::new(input))
                            .await;
                        if let Ok(mut rep) = trace_response(resp) {
                            tx.send(Ok(OpenFileOutput {
                                obj_rep: rep.reps.pop(),
                            }))
                            .await
                            .unwrap();
                        }
                    }
                    Err(e) => error!("{}", e),
                }
            }
        });
        Ok(Response::new(rx))
    }

    #[instrument]
    async fn create_walls(
        &self,
        request: Request<CreateWallsInput>,
    ) -> Result<Response<CreateWallsOutput>, Status> {
        let msg = request.into_inner();
        let mut ops_client = common::operations_client(self.ops_url.clone()).await?;
        let mut submit_client = common::submit_client(self.submit_url.clone()).await?;
        let prefix = Prefix::new(msg.prefix)?;
        let mut walls = Vec::new();
        for wall in msg.walls {
            let wall_msg = operations::WallMsg {
                first_pt: wall.first_pt,
                second_pt: wall.second_pt,
                width: wall.width,
                height: wall.height,
            };
            info!("Creating wall {:?}", wall_msg);
            walls.push(wall_msg);
        }
        let resp = ops_client
            .create_walls(TracedRequest::new(operations::CreateWallsInput { walls }))
            .await;
        let objects = trace_response(resp)?;
        let mut changes = Vec::new();
        let mut ids = Vec::new();
        for obj in objects.walls.into_iter() {
            ids.push(obj.id.clone());
            changes.push(common::add(&prefix.user, obj));
        }

        let resp = submit_client
            .submit_changes(TracedRequest::new(submit::SubmitChangesInput {
                file: prefix.file,
                user: prefix.user,
                offset: prefix.offset,
                changes,
            }))
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

    #[instrument]
    async fn move_objects(
        &self,
        request: Request<MoveObjectsInput>,
    ) -> Result<Response<MoveObjectsOutput>, Status> {
        let msg = request.into_inner();
        let mut obj_client = common::objects_client(self.obj_url.clone()).await?;
        let mut ops_client = common::operations_client(self.ops_url.clone()).await?;
        let mut submit_client = common::submit_client(self.submit_url.clone()).await?;
        let prefix = Prefix::new(msg.prefix)?;

        let objects = common::get_objects(
            &mut obj_client,
            &prefix.file,
            msg.obj_ids,
            prefix.offset,
            false,
        )
        .await?;

        let resp = ops_client
            .move_objects(TracedRequest::new(operations::MoveObjectsInput {
                objects,
                delta: msg.delta,
            }))
            .await;
        let objects = trace_response(resp)?;
        let mut changes = Vec::new();
        for obj in objects.objects {
            changes.push(common::modify(&prefix.user, obj));
        }
        let offset = common::submit_changes(
            &mut submit_client,
            prefix.file,
            prefix.user,
            prefix.offset,
            changes,
        )
        .await?;
        Ok(Response::new(MoveObjectsOutput { offset }))
    }

    async fn join_objects_at_point(
        &self,
        request: Request<JoinObjectsAtPointInput>,
    ) -> Result<Response<JoinObjectsAtPointOutput>, Status> {
        let msg = request.into_inner();
        let mut obj_client = common::objects_client(self.obj_url.clone()).await?;
        let mut ops_client = common::operations_client(self.ops_url.clone()).await?;
        let mut submit_client = common::submit_client(self.submit_url.clone()).await?;
        let prefix = Prefix::new(msg.prefix)?;

        let mut objects = common::get_objects(
            &mut obj_client,
            &prefix.file,
            vec![msg.first_id, msg.second_id],
            prefix.offset,
            true,
        )
        .await?;

        let second_opt = objects.pop();
        let first_opt = objects.pop();

        let resp = ops_client
            .join_objects(TracedRequest::new(operations::JoinObjectsInput {
                first_obj: first_opt,
                second_obj: second_opt,
                first_wants: object_state::ref_id_msg::RefType::ProfilePoint as i32,
                second_wants: object_state::ref_id_msg::RefType::ProfilePoint as i32,
                guess: msg.guess,
            }))
            .instrument(info_span!("join_objects"))
            .await;
        let output = trace_response(resp)?;
        let mut changes = Vec::new();
        if let Some(first_obj) = output.first_obj {
            changes.push(common::modify(&prefix.user, first_obj));
        }
        if let Some(second_obj) = output.second_obj {
            changes.push(common::modify(&prefix.user, second_obj));
        }
        let offset = common::submit_changes(
            &mut submit_client,
            prefix.file,
            prefix.user,
            prefix.offset,
            changes,
        )
        .await?;
        Ok(Response::new(JoinObjectsAtPointOutput { offset }))
    }

    #[instrument]
    async fn delete_objects(
        &self,
        request: Request<DeleteObjectsInput>,
    ) -> Result<Response<DeleteObjectsOutput>, Status> {
        let msg = request.into_inner();
        let mut submit_client = common::submit_client(self.submit_url.clone()).await?;
        let prefix = Prefix::new(msg.prefix)?;

        let mut changes = Vec::new();
        for obj_id in msg.obj_ids {
            changes.push(common::delete(&prefix.user, obj_id));
        }
        let offset = common::submit_changes(
            &mut submit_client,
            prefix.file,
            prefix.user,
            prefix.offset,
            changes,
        )
        .await?;
        Ok(Response::new(DeleteObjectsOutput { offset }))
    }

    #[instrument]
    async fn create_sheet(
        &self,
        request: Request<CreateSheetInput>,
    ) -> Result<Response<CreateSheetOutput>, Status> {
        let mut ops_client = common::operations_client(self.ops_url.clone()).await?;
        let mut submit_client = common::submit_client(self.submit_url.clone()).await?;
        let msg = request.into_inner();
        let prefix = Prefix::new(msg.prefix)?;
        let ops_sheet = operations::CreateSheetInput {
            name: msg.name,
            print_size: msg.print_size,
        };
        let resp = ops_client.create_sheet(TracedRequest::new(ops_sheet)).await;
        let object = trace_response(resp)?;
        match object.sheet {
            Some(obj_msg) => {
                let id = obj_msg.id.clone();
                let change = common::add(&prefix.user, obj_msg);
                let resp = submit_client
                    .submit_changes(TracedRequest::new(submit::SubmitChangesInput {
                        file: prefix.file,
                        user: prefix.user,
                        offset: prefix.offset,
                        changes: vec![change],
                    }))
                    .await;
                let mut output = trace_response(resp)?;
                match output.offsets.pop() {
                    Some(offset) => Ok(Response::new(CreateSheetOutput {
                        sheet_id: id,
                        offset,
                    })),
                    None => Err(Status::out_of_range(
                        "No offsets received from submit service",
                    )),
                }
            }
            None => Err(Status::not_found(
                "No sheet returned from operations service",
            )),
        }
    }

    #[instrument]
    async fn create_viewport(
        &self,
        request: Request<CreateViewportInput>,
    ) -> Result<Response<CreateViewportOutput>, Status> {
        let msg = request.into_inner();
        let mut ops_client = common::operations_client(self.ops_url.clone()).await?;
        let mut submit_client = common::submit_client(self.submit_url.clone()).await?;
        let prefix = Prefix::new(msg.prefix)?;
        let view_type = match msg.view_type {
            Some(create_viewport_input::ViewType::Top(msg)) => {
                operations::create_viewport_input::ViewType::Top(msg)
            }
            Some(create_viewport_input::ViewType::Front(msg)) => {
                operations::create_viewport_input::ViewType::Front(msg)
            }
            Some(create_viewport_input::ViewType::Left(msg)) => {
                operations::create_viewport_input::ViewType::Left(msg)
            }
            Some(create_viewport_input::ViewType::Right(msg)) => {
                operations::create_viewport_input::ViewType::Right(msg)
            }
            Some(create_viewport_input::ViewType::Back(msg)) => {
                operations::create_viewport_input::ViewType::Back(msg)
            }
            Some(create_viewport_input::ViewType::Bottom(msg)) => {
                operations::create_viewport_input::ViewType::Bottom(msg)
            }
            Some(create_viewport_input::ViewType::Custom(msg)) => {
                operations::create_viewport_input::ViewType::Custom(operations::CustomViewMsg {
                    camera_pos: msg.camera_pos,
                    target: msg.target,
                })
            }
            None => return Err(tonic::Status::invalid_argument("No view type passed in")),
        };
        let ops_viewport = operations::CreateViewportInput {
            sheet_id: msg.sheet_id,
            view_type: Some(view_type),
            origin: msg.origin,
            scale: msg.scale,
        };
        let resp = ops_client
            .create_viewport(TracedRequest::new(ops_viewport))
            .await;
        let object = trace_response(resp)?;
        match object.viewport {
            Some(obj_msg) => {
                let id = obj_msg.id.clone();
                let change = common::add(&prefix.user, obj_msg);
                let resp = submit_client
                    .submit_changes(TracedRequest::new(submit::SubmitChangesInput {
                        file: prefix.file,
                        user: prefix.user,
                        offset: prefix.offset,
                        changes: vec![change],
                    }))
                    .await;
                let mut output = trace_response(resp)?;
                match output.offsets.pop() {
                    Some(offset) => Ok(Response::new(CreateViewportOutput {
                        viewport_id: id,
                        offset,
                    })),
                    None => Err(Status::out_of_range(
                        "No offsets received from submit service",
                    )),
                }
            }
            None => Err(Status::not_found(
                "No viewport returned from operations service",
            )),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let jaeger_url = std::env::var("JAEGER_URL").unwrap();
    let undo_url = std::env::var("UNDO_URL").unwrap().parse().unwrap();
    let obj_url = std::env::var("OBJECTS_URL").unwrap().parse().unwrap();
    let ops_url = std::env::var("OPS_URL").unwrap().parse().unwrap();
    let submit_url = std::env::var("SUBMIT_URL").unwrap().parse().unwrap();
    let rep_cache_url = std::env::var("REP_CACHE_URL").unwrap().parse().unwrap();
    trace_lib::init_tracer(&jaeger_url, "api")?;
    let svc = api_server::ApiServer::new(ApiService {
        undo_url,
        obj_url,
        ops_url,
        submit_url,
        rep_cache_url,
    });
    println!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
