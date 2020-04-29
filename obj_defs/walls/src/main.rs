use log::*;
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};

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

fn get_third_pt(pt_opt: &Option<Point3Msg>, height: f64) -> Option<Point3Msg> {
    match pt_opt {
        Some(pt) => Some(Point3Msg {
            x: pt_opt.x,
            y: pt_opt.y,
            z: pt_opt.z + height,
        }),
        None => None,
    }
}
pub fn minimum_of_list(list: &Vec<f64>) -> f64 {
    let mut iter = list.iter();
    let init = iter.next().unwrap();
    let result = iter.fold(init, |acc, x| {
        // return None if x is NaN
        let cmp = x.partial_cmp(&acc);
        if let Some(std::cmp::Ordering::Less) = cmp {
            x
        } else {
            acc
        }
    });
    *result
}

pub fn maximum_of_list(list: &Vec<f64>) -> f64 {
    let mut iter = list.into_iter();
    let init = iter.next().unwrap();
    let result = iter.fold(init, |acc, x| {
        // return None if x is NaN
        let cmp = x.partial_cmp(&acc);
        if let Some(std::cmp::Ordering::Greater) = cmp {
            x
        } else {
            acc
        }
    });
    *result
}

pub fn offset_line(
    first_pt: &Point3Msg,
    second_pt: &Point3Msg,
    width: f64,
) -> (Point3Msg, Point3Msg, Point3Msg, Point3Msg) {
    let dir = second_pt - first_pt;
    let perp = dir.cross(Vector3f::unit_z()).normalize();
    let offset = perp * width;
    let first = first_pt + offset;
    let second = second_pt + offset;
    let third = second_pt - offset;
    let fourth = first_pt - offset;
    (first, second, third, fourth)
}

pub fn get_axis_aligned_bound_box(
    first_pt: &Point3Msg,
    second_pt: &Point3Msg,
    width: f64,
    height: f64,
) -> Cube {
    let (first, second, third, fourth) = offset_line(first_pt, second_pt, width);
    let vert_offset = Vector3f::new(0.0, 0.0, height);
    let fifth = first + vert_offset;
    let sixth = second + vert_offset;
    let seventh = third + vert_offset;
    let eighth = fourth + vert_offset;
    let x_vals = vec![
        first.x, second.x, third.x, fourth.x, fifth.x, sixth.x, seventh.x, eighth.x,
    ];
    let y_vals = vec![
        first.y, second.y, third.y, fourth.y, fifth.y, sixth.y, seventh.y, eighth.y,
    ];
    let z_vals = vec![
        first.z, second.z, third.z, fourth.z, fifth.z, sixth.z, seventh.z, eighth.z,
    ];
    let bottom_left = Point3f::new(
        minimum_of_list(&x_vals).unwrap(),
        minimum_of_list(&y_vals).unwrap(),
        minimum_of_list(&z_vals).unwrap(),
    );
    let top_right = Point3f::new(
        maximum_of_list(&x_vals).unwrap(),
        maximum_of_list(&y_vals).unwrap(),
        maximum_of_list(&z_vals).unwrap(),
    );
    Cube {
        bottom_left,
        top_right,
    }
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
        for wall in msg.walls {
            let output = ObjectMsg {
                dependencies: None,
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
                        planes: vec![OptionPlaneMsg {
                            plane: Some(PlaneMsg {
                                first: wall.first_pt.clone(),
                                second: wall.second_pt.clone(),
                                third: get_third_pt(&wall.second_pt, wall.height),
                            }),
                        }],
                    }),
                    bbox: AxisAlignedBboxMsg {},
                }),
            };
            results.push(output);
        }
        Ok(Response::new(CreateWallsOutput { walls: results }))
    }

    async fn recalculate(
        &self,
        request: Request<RecalculateInput>,
    ) -> Result<Response<RecalculateOutput>, Status> {
        unimplemented!();
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
