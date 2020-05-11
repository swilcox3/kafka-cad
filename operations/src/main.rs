use log::*;
use math::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tonic::transport::{Channel, Server};
use tonic::{Request, Response, Status};

pub use geom::*;
pub use object_state::*;

mod obj_traits;
use obj_traits::*;

mod obj_defs;
use obj_defs::*;

mod geom_kernel {
    tonic::include_proto!("geom_kernel");
}
use geom_kernel::*;
pub type GeomKernel = geometry_kernel_client::GeometryKernelClient<Channel>;

mod operations {
    tonic::include_proto!("operations");
}
use operations::*;

mod representation {
    tonic::include_proto!("representation");
}
use representation::*;

#[derive(Debug, Error)]
pub enum OpsError {
    #[error("Object {0:?} is incorrect type, expected {1:?}")]
    ObjWrongType(String, String),
    #[error("Object {0:?} lacks trait {1}")]
    ObjLacksTrait(String, String),
    #[error("JSON error")]
    JsonError(#[from] serde_json::Error),
    #[error("Prost encode error")]
    ProstEncodeError(#[from] prost::EncodeError),
    #[error("Prost decode error")]
    ProstDecodeError(#[from] prost::DecodeError),
    #[error("{0}")]
    StatusError(#[from] Status),
    #[error("Invalid argument")]
    InvalidArgs,
    #[error("Unknown error {0}")]
    Other(String),
}

impl Into<tonic::Status> for OpsError {
    fn into(self) -> tonic::Status {
        let msg = format!("{}", self);
        let code = match self {
            OpsError::JsonError(..)
            | OpsError::ProstEncodeError(..)
            | OpsError::ProstDecodeError(..)
            | OpsError::Other(..) => tonic::Code::Internal,
            OpsError::ObjWrongType(..) | OpsError::ObjLacksTrait(..) | OpsError::InvalidArgs => {
                tonic::Code::InvalidArgument
            }
            OpsError::StatusError(status) => status.code(),
        };
        tonic::Status::new(code, msg)
    }
}

pub fn to_status<T: Into<OpsError>>(err: T) -> tonic::Status {
    let obj_error: OpsError = err.into();
    obj_error.into()
}

pub fn other_error<T: std::fmt::Debug>(err: T) -> OpsError {
    OpsError::Other(format!("{:?}", err))
}

struct OpsService {
    geom_url: String,
}

#[tonic::async_trait]
impl operations_server::Operations for OpsService {
    async fn create_walls(
        &self,
        request: Request<CreateWallsInput>,
    ) -> Result<Response<CreateWallsOutput>, Status> {
        unimplemented!();
    }

    async fn update_objects(
        &self,
        request: Request<UpdateObjectsInput>,
    ) -> Result<Response<UpdateObjectsOutput>, Status> {
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
    let geom_url = std::env::var("GEOM_URL").unwrap();

    let svc = operations_server::OperationsServer::new(OpsService { geom_url });

    info!("Running on {:?}", run_url);
    Server::builder()
        .add_service(svc)
        .serve(run_url)
        .await
        .unwrap();
    Ok(())
}
