use log::*;
use operations::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

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

struct OperationsService {
    geom_url: String,
}

#[tonic::async_trait]
impl operations_server::Operations for OperationsService {
    async fn create_walls(
        &self,
        request: Request<CreateWallsInput>,
    ) -> Result<Response<CreateWallsOutput>, Status> {
        let walls_msg = request.get_ref();
        info!("Create walls: {:?}", walls_msg);
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

    async fn update_objects(
        &self,
        request: Request<UpdateObjectsInput>,
    ) -> Result<Response<UpdateObjectsOutput>, Status> {
        let update_msg = request.get_ref();
        info!("Update objects: {:?}", update_msg);
        let refers = from_ref_msgs(&update_msg.obj_refs)?;
        let mut objs = get_map_from_change_msgs(&update_msg.objects)?;
        operations::update_all(&mut objs, refers);
        let changes = to_change_msgs(&update_msg.objects, &objs).map_err(to_status)?;
        Ok(Response::new(UpdateObjectsOutput { objects: changes }))
    }

    async fn client_representation(
        &self,
        request: Request<ClientRepresentationInput>,
    ) -> Result<Response<ClientRepresentationOutput>, Status> {
        let repr_msg = request.get_ref();
        info!("Client representation: {:?}", repr_msg);
        let mut geom_conn =
            new_geom_conn(self.geom_url.clone())
                .await
                .map_err(to_status)?;
        let changes = from_change_msgs(&repr_msg.objects)?;
        let mut outputs = Vec::new();
        for change in changes {
            let (output, views_opt) = match change {
                Change::Add { obj } | Change::Modify { obj } => {
                    get_obj_update_info(&mut geom_conn, &obj)
                        .await
                        .map_err(to_status)?
                }
                Change::Delete { .. } => (UpdateOutput::Delete, None),
            };
            outputs.push(encode_update_output(output, views_opt));
        }
        Ok(Response::new(ClientRepresentationOutput { outputs }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let geom_url = std::env::var("GEOM_URL").unwrap();
    let svc = operations_server::OperationsServer::new(OperationsService { geom_url });

    info!("Running on {:?}", run_url);
    Server::builder().add_service(svc).serve(run_url).await?;
    Ok(())
}
