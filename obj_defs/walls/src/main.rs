use geom::*;
use log::*;
use math::*;
use object_state::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

mod geom_kernel {
    tonic::include_proto!("geom_kernel");
}
use geom_kernel::*;

mod walls {
    tonic::include_proto!("walls");
}
use walls::*;

mod representation {
    tonic::include_proto!("representation");
}
use representation::*;

mod repr;
mod wall;
use wall::*;

mod obj_defs {
    tonic::include_proto!("obj_defs");
}
use obj_defs::*;

fn to_point3f(pt: &Point3Msg) -> Point3f {
    Point3f::new(pt.x, pt.y, pt.z)
}

fn to_point3msg(pt: &Point3f) -> Point3Msg {
    Point3Msg {
        x: pt.x,
        y: pt.y,
        z: pt.z,
    }
}

fn bbox(
    pt_opt_1: &Option<Point3Msg>,
    pt_opt_2: &Option<Point3Msg>,
    width: f64,
    height: f64,
) -> Option<AxisAlignedBBoxMsg> {
    let mut result = None;
    if let Some(pt_1) = pt_opt_1 {
        if let Some(pt_2) = pt_opt_2 {
            let bbox =
                get_axis_aligned_bound_box(&to_point3f(pt_1), &to_point3f(pt_2), width, height);
            result = Some(AxisAlignedBBoxMsg {
                bottom_left: Some(to_point3msg(&bbox.bottom_left)),
                top_right: Some(to_point3msg(&bbox.top_right)),
            });
        }
    }
    result
}

#[derive(Debug, Error)]
pub enum WallError {
    #[error("Couldn't connect to geom kernel: {0}")]
    ConnectError(#[from] tonic::transport::Error),
    #[error("{0}")]
    StatusError(#[from] tonic::Status),
    #[error("JSON error: {0}")]
    JSONError(#[from] serde_json::Error),
    #[error("Missing arguments")]
    InvalidArgs,
}

impl Into<tonic::Status> for WallError {
    fn into(self) -> tonic::Status {
        let msg = format!("{}", self);
        let code = match self {
            WallError::JSONError(..) => tonic::Code::Internal,
            WallError::StatusError(status) => status.code(),
            WallError::ConnectError(..) => tonic::Code::Unavailable,
            WallError::InvalidArgs => tonic::Code::InvalidArgument,
        };
        tonic::Status::new(code, msg)
    }
}

pub fn to_status<T: Into<WallError>>(err: T) -> tonic::Status {
    let repr_error: WallError = err.into();
    repr_error.into()
}

struct WallsService {}

#[tonic::async_trait]
impl walls_server::Walls for WallsService {
    async fn create_walls(
        &self,
        request: Request<CreateWallsInput>,
    ) -> Result<Response<CreateWallsOutput>, Status> {
        let msg = request.get_ref();
        info!("Create walls: {:?}", msg);
        let mut results = Vec::new();
        for wall in &msg.walls {
            let output = ObjectMsg {
                obj_url: String::from("walls"),
                dependencies: Some(DependenciesMsg {
                    references: vec![
                        OptionReferenceMsg {
                            reference: Some(ReferenceMsg {
                                owner: Some(RefIdMsg {
                                    id: wall.id.clone(),
                                    ref_type: ref_id_msg::RefType::ProfileLine as i32,
                                    index: 0,
                                }),
                                other: Some(RefIdMsg {
                                    id: wall.id.clone(),
                                    ref_type: ref_id_msg::RefType::ProfilePoint as i32,
                                    index: 0,
                                }),
                                update_type: Some(reference_msg::UpdateType::Equals(
                                    UpdateTypeEqualsMsg {
                                        owner_index: 0,
                                        other_index: 0,
                                    },
                                )),
                            }),
                        },
                        OptionReferenceMsg {
                            reference: Some(ReferenceMsg {
                                owner: Some(RefIdMsg {
                                    id: wall.id.clone(),
                                    ref_type: ref_id_msg::RefType::ProfileLine as i32,
                                    index: 0,
                                }),
                                other: Some(RefIdMsg {
                                    id: wall.id.clone(),
                                    ref_type: ref_id_msg::RefType::ProfilePoint as i32,
                                    index: 1,
                                }),
                                update_type: Some(reference_msg::UpdateType::Equals(
                                    UpdateTypeEqualsMsg {
                                        owner_index: 1,
                                        other_index: 0,
                                    },
                                )),
                            }),
                        },
                    ],
                }),
                results: Some(ResultsMsg {
                    visible: true,
                    profile: Some(ProfileMsg {
                        points: vec![
                            OptionPoint3Msg::new(wall.first_pt.clone()),
                            OptionPoint3Msg::new(wall.second_pt.clone()),
                        ],
                        lines: vec![OptionLineMsg {
                            line: Some(LineMsg {
                                first: wall.first_pt.clone(),
                                second: wall.second_pt.clone(),
                            }),
                        }],
                        planes: Vec::new(),
                    }),
                    bbox: bbox(&wall.first_pt, &wall.second_pt, wall.width, wall.height),
                    properties: Some(PropertiesMsg {
                        prop_json: serde_json::to_string(&WallProps {
                            width: wall.width,
                            height: wall.height,
                        })
                        .unwrap(),
                    }),
                }),
                obj_data: Vec::new(),
            };
            results.push(output);
        }
        Ok(Response::new(CreateWallsOutput { walls: results }))
    }
}

struct ObjDefService {
    geom_url: String,
}

#[tonic::async_trait]
impl obj_def_server::ObjDef for ObjDefService {
    async fn recalculate(
        &self,
        request: Request<RecalculateInput>,
    ) -> Result<Response<RecalculateOutput>, Status> {
        let mut msg = request.into_inner();
        info!("Recalculate: {:?}", msg);
        for obj in &mut msg.objects {
            let wall = Wall::from_obj_msg(&obj).map_err(to_status)?;
            if let Some(bbox_msg) = obj.get_bbox_mut() {
                *bbox_msg = bbox(
                    &Some(wall.first_pt),
                    &Some(wall.second_pt),
                    wall.width,
                    wall.height,
                )
                .unwrap();
            }
        }
        Ok(Response::new(RecalculateOutput {
            objects: msg.objects,
        }))
    }

    async fn client_representation(
        &self,
        request: Request<ClientRepresentationInput>,
    ) -> Result<Response<ClientRepresentationOutput>, Status> {
        let msg = request.into_inner();
        let mut output = None;
        info!("Client representation: {:?}", msg);
        if let Some(object) = msg.object {
            let wall = Wall::from_obj_msg(&object).map_err(to_status)?;
            let results = repr::get_repr(self.geom_url.clone(), wall)
                .await
                .map_err(to_status)?;
            output = Some(results);
        }
        Ok(Response::new(ClientRepresentationOutput { output }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let geom_url = std::env::var("GEOM_URL").unwrap().parse().unwrap();
    let wall_svc = walls_server::WallsServer::new(WallsService {});
    let def_svc = obj_def_server::ObjDefServer::new(ObjDefService { geom_url });

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(wall_svc)
        .add_service(def_svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
