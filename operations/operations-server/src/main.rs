use operations::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use trace_lib::*;
use tracing::*;
use tracing_futures::Instrument;

mod representation {
    tonic::include_proto!("representation");
}

mod object_state {
    tonic::include_proto!("object_state");
}
use object_state::*;

mod geom {
    tonic::include_proto!("geom");
}
use geom::*;

mod ops_proto {
    tonic::include_proto!("operations");
}

mod obj_convert;

use obj_convert::*;
use ops_proto::*;

#[derive(Debug)]
struct OperationsService {
    geom_url: String,
}

#[tonic::async_trait]
impl operations_server::Operations for OperationsService {
    #[instrument]
    async fn create_walls(
        &self,
        request: Request<CreateWallsInput>,
    ) -> Result<Response<CreateWallsOutput>, Status> {
        let walls_msg = request.get_ref();
        propagate_trace(request.metadata());
        let mut results = Vec::new();
        for wall_msg in &walls_msg.walls {
            let wall = Box::new(to_wall(
                &wall_msg.first_pt,
                &wall_msg.second_pt,
                wall_msg.width,
                wall_msg.height,
            )?) as DataBox;
            results.push(to_object_msg(&wall).map_err(to_status)?);
        }
        Ok(Response::new(CreateWallsOutput { walls: results }))
    }

    #[instrument]
    async fn move_objects(
        &self,
        request: Request<MoveObjectsInput>,
    ) -> Result<Response<MoveObjectsOutput>, Status> {
        let msg = request.get_ref();
        propagate_trace(request.metadata());
        let mut objects = from_obj_msgs(&msg.objects)?;
        let delta = to_vector_3f(&msg.delta)?;
        operations::move_objects(&mut objects, &delta);
        let obj_msgs = to_object_msgs(&objects)?;
        Ok(Response::new(MoveObjectsOutput { objects: obj_msgs }))
    }

    #[instrument]
    async fn join_object_to_other(
        &self,
        request: Request<JoinObjectToOtherInput>,
    ) -> Result<Response<JoinObjectToOtherOutput>, Status> {
        let msg = request.get_ref();
        propagate_trace(request.metadata());
        let mut obj = from_obj_msg_opt(&msg.to_join)?;
        let other_obj = from_obj_msg_opt(&msg.join_to)?;
        let ref_type = from_ref_type_msg(msg.looking_for)?;
        let guess = to_point_3f(&msg.guess)?;
        operations::snap_to_ref(&mut obj, &other_obj, ref_type, &guess).map_err(to_status)?;
        let obj_msg = to_object_msg(&obj).map_err(to_status)?;
        Ok(Response::new(JoinObjectToOtherOutput {
            joined: Some(obj_msg),
        }))
    }

    #[instrument]
    async fn join_objects(
        &self,
        request: Request<JoinObjectsInput>,
    ) -> Result<Response<JoinObjectsOutput>, Status> {
        let msg = request.get_ref();
        propagate_trace(request.metadata());
        let mut first_obj = from_obj_msg_opt(&msg.first_obj)?;
        let mut second_obj = from_obj_msg_opt(&msg.second_obj)?;
        let first_wants = from_ref_type_msg(msg.first_wants)?;
        let second_wants = from_ref_type_msg(msg.second_wants)?;
        let guess = to_point_3f(&msg.guess)?;
        operations::join_refs(
            &mut first_obj,
            &mut second_obj,
            first_wants,
            second_wants,
            &guess,
        )
        .map_err(to_status)?;
        let first_msg = to_object_msg(&first_obj).map_err(to_status)?;
        let second_msg = to_object_msg(&second_obj).map_err(to_status)?;
        Ok(Response::new(JoinObjectsOutput {
            first_obj: Some(first_msg),
            second_obj: Some(second_msg),
        }))
    }

    #[instrument]
    async fn update_objects(
        &self,
        request: Request<UpdateObjectsInput>,
    ) -> Result<Response<UpdateObjectsOutput>, Status> {
        propagate_trace(request.metadata());
        let update_msg = request.get_ref();
        let refers = from_ref_msgs(&update_msg.obj_refs)?;
        let mut objs = get_map_from_change_msgs(&update_msg.objects)?;
        operations::update_all(&mut objs, refers);
        let changes = to_change_msgs(&update_msg.objects, &objs).map_err(to_status)?;
        Ok(Response::new(UpdateObjectsOutput { objects: changes }))
    }

    #[instrument]
    async fn client_representation(
        &self,
        request: Request<ClientRepresentationInput>,
    ) -> Result<Response<ClientRepresentationOutput>, Status> {
        propagate_trace(request.metadata());
        let repr_msg = request.get_ref();
        let mut geom_conn = new_geom_conn(self.geom_url.clone())
            .instrument(info_span!("new_geom_conn"))
            .await
            .map_err(to_status)?;
        let changes = from_change_msgs(&repr_msg.objects)?;
        let mut outputs = Vec::new();
        for change in changes {
            let (output, views_opt) = match change {
                Change::Add { obj } | Change::Modify { obj } => {
                    get_obj_update_info(&mut geom_conn, &obj)
                        .instrument(info_span!("get_obj_update_info"))
                        .await
                        .map_err(to_status)?
                }
                Change::Delete { .. } => (UpdateOutput::Delete, None),
            };
            outputs.push(encode_update_output(output, views_opt));
        }
        Ok(Response::new(ClientRepresentationOutput { outputs }))
    }

    #[instrument]
    async fn create_sheet(
        &self,
        request: Request<CreateSheetInput>,
    ) -> Result<Response<CreateSheetOutput>, Status> {
        propagate_trace(request.metadata());
        let sheet_msg = request.into_inner();
        let sheet = Box::new(to_sheet(sheet_msg)?) as DataBox;
        let sheet_msg = to_object_msg(&sheet).map_err(to_status)?;
        Ok(Response::new(CreateSheetOutput {
            sheet: Some(sheet_msg),
        }))
    }

    #[instrument]
    async fn create_viewport(
        &self,
        request: Request<CreateViewportInput>,
    ) -> Result<Response<CreateViewportOutput>, Status> {
        propagate_trace(request.metadata());
        let viewport_msg = request.into_inner();
        let viewport = Box::new(to_viewport(viewport_msg)?) as DataBox;
        let viewport_msg = to_object_msg(&viewport).map_err(to_status)?;
        Ok(Response::new(CreateViewportOutput {
            viewport: Some(viewport_msg),
        }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let jaeger_url = std::env::var("JAEGER_URL").unwrap();
    let geom_url = std::env::var("GEOM_URL").unwrap();
    trace_lib::init_tracer(&jaeger_url, "operations")?;
    let svc = operations_server::OperationsServer::new(OperationsService { geom_url });

    println!("Running on {:?}", run_url);
    Server::builder().add_service(svc).serve(run_url).await?;
    Ok(())
}
