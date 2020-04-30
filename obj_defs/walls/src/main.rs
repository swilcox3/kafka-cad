use log::*;
use math::*;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use serde_json::json;

mod walls {
    include!(concat!(env!("OUT_DIR"), "/walls.rs"));
}
use walls::*;

mod object_state {
    include!(concat!(env!("OUT_DIR"), "/object_state.rs"));
    impl OptionPoint3Msg {
        pub fn new(pt: Option<Point3Msg>) -> OptionPoint3Msg {
            OptionPoint3Msg { pt }
        }
    }
}
use object_state::*;

mod representation {
    include!(concat!(env!("OUT_DIR"), "/representation.rs"));
}
use representation::*;

mod obj_defs {
    include!(concat!(env!("OUT_DIR"), "/obj_defs.rs"));
}
use obj_defs::*;

fn to_point3f(pt: &Point3Msg) -> Point3f {
    Point3f::new(pt.x, pt.y, pt.z)
}

fn to_point3msg(pt: &Point3f) -> Point3Msg {
    Point3Msg {
        x: pt.x, 
        y: pt.y, 
        z: pt.z
    }
}

fn get_third_pt(pt_opt: &Option<Point3Msg>, height: f64) -> Option<Point3Msg> {
    match pt_opt {
        Some(pt) => Some(Point3Msg {
            x: pt.x,
            y: pt.y,
            z: pt.z + height,
        }),
        None => None,
    }
}

fn bbox(pt_opt_1: &Option<Point3Msg>, pt_opt_2: &Option<Point3Msg>, width: f64, height: f64) -> Option<AxisAlignedBBoxMsg> {
    let mut result = None;
    if let Some(pt_1) = pt_opt_1 {
        if let Some(pt_2) = pt_opt_2 {
            let bbox = get_axis_aligned_bound_box(&to_point3f(pt_1), &to_point3f(pt_2), width, height);
            result = Some(AxisAlignedBBoxMsg {
                bottom_left: Some(to_point3msg(&bbox.bottom_left)),
                top_right: Some(to_point3msg(&bbox.top_right))
            });
        }
    }
    result
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
                dependencies: Some(DependenciesMsg {
                    references: vec![
                        OptionReferenceMsg {
                            reference: Some(ReferenceMsg {
                                owner: Some(RefIdMsg {
                                    id: wall.id.clone(),
                                    ref_type: ref_id_msg::RefType::ProfileLine as i32,
                                    index: 0
                                }),
                                other: Some(RefIdMsg {
                                    id: wall.id.clone(),
                                    ref_type: ref_id_msg::RefType::ProfilePoint as i32,
                                    index: 0
                                }),
                                update_type: Some(reference_msg::UpdateType::Equals(UpdateTypeEqualsMsg{
                                    owner_index: 0,
                                    other_index: 0
                                }))
                            })
                        },
                        OptionReferenceMsg {
                            reference: Some(ReferenceMsg {
                                owner: Some(RefIdMsg {
                                    id: wall.id.clone(),
                                    ref_type: ref_id_msg::RefType::ProfileLine as i32,
                                    index: 0
                                }),
                                other: Some(RefIdMsg {
                                    id: wall.id.clone(),
                                    ref_type: ref_id_msg::RefType::ProfilePoint as i32,
                                    index: 1
                                }),
                                update_type: Some(reference_msg::UpdateType::Equals(UpdateTypeEqualsMsg{
                                    owner_index: 1,
                                    other_index: 0
                                }))
                            })
                        },
                    ]
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
                        prop_json: json!({
                            "Width": wall.width,
                            "Height": wall.height
                        }).to_string()
                    })
                }),
                obj_data: Vec::new()
            };
            results.push(output);
        }
        Ok(Response::new(CreateWallsOutput { walls: results }))
    }

    async fn recalculate(
        &self,
        request: Request<RecalculateInput>,
    ) -> Result<Response<RecalculateOutput>, Status> {
        let msg = request.into_inner();
        info!("Recalculate: {:?}", msg);
        //Walls don't have any inner data to update
        Ok(Response::new(RecalculateOutput{ objects: msg.objects }))
    }

    async fn client_representation(
        &self,
        request: Request<ClientRepresentationInput>,
    ) -> Result<Response<ClientRepresentationOutput>, Status> {
        unimplemented!();
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap().parse().unwrap();
    let svc = walls_server::WallsServer::new(WallsService {});

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
