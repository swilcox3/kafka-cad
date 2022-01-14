use anyhow::{anyhow, Result};
use tonic::transport::Channel;
use tonic::Request;

mod geom {
    tonic::include_proto!("geom");
}
pub use geom::*;

mod representation {
    tonic::include_proto!("representation");
}
pub use representation::*;

pub mod api {
    tonic::include_proto!("api");
}
pub use api::*;

pub mod producer;
pub mod subscriber;
