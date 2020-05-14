use log::*;
use prost::Message;
use thiserror::Error;
use tonic::Request;

mod geom {
    tonic::include_proto!("geom");
}

mod object_state {
    tonic::include_proto!("object_state");
}
use object_state::*;

mod representation {
    tonic::include_proto!("representation");
}
use representation::*;

mod operations {
    tonic::include_proto!("operations");
}
use operations::*;

mod consume;
mod produce;

#[derive(Debug, Error)]
pub enum RepresentationError {
    #[error("Kafka error: {0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
    #[error("String error: {0}")]
    StringError(#[from] std::str::Utf8Error),
    #[error("Message from partition {partition} and offset {offset} has no payload")]
    PayloadError { partition: i32, offset: i64 },
    #[error("Message from partition {partition} and offset {offset} has no file key set")]
    FileError { partition: i32, offset: i64 },
    #[error("Transport error: {0}")]
    TransportError(#[from] tonic::transport::Error),
    #[error("Service error: {0}")]
    ServiceError(#[from] tonic::Status),
    #[error("Prost encode error: {0}")]
    ProstEncodeError(#[from] prost::EncodeError),
    #[error("Prost decode error: {0}")]
    ProstDecodeError(#[from] prost::DecodeError),
}

async fn call_service(ops_url: String, object: ChangeMsg) -> Result<Option<UpdateOutputMsg>, RepresentationError> {
    let mut client = operations::operations_client::OperationsClient::connect(ops_url).await?;
    let mut representation = client
        .client_representation(Request::new(ClientRepresentationInput {
            objects: vec![object],
        }))
        .await?
        .into_inner();
    Ok(representation.outputs.pop())
}

pub async fn calc_representation(
    broker: &str,
    topic: &str,
    file: &str,
    ops_url: String,
    msg: &[u8],
) -> Result<(), RepresentationError> {
    let change = object_state::ChangeMsg::decode(msg)?;
    let repr_opt = call_service(ops_url, change).await?;
    if let Some(repr) = repr_opt {
        produce::submit_representations(broker, topic, file, repr).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();
    let obj_topic = std::env::var("OBJ_TOPIC").unwrap();
    let repr_topic = std::env::var("REPR_TOPIC").unwrap();
    let ops_url = std::env::var("OPS_URL").unwrap();
    consume::start_consume_stream(broker, group, obj_topic, repr_topic, ops_url).await;
    return Ok(());
}