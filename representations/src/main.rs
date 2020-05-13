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

async fn call_service(object: ObjectMsg) -> Result<Option<UpdateOutputMsg>, RepresentationError> {
    let mut client = obj_defs::obj_def_client::ObjDefClient::connect(url).await?;
    let representation = client
        .client_representation(Request::new(ClientRepresentationInput {
            object: Some(object),
        }))
        .await?
        .into_inner();
    Ok(representation.output)
}

pub async fn calc_representation(
    broker: &str,
    topic: &str,
    file: &str,
    msg: &[u8],
) -> Result<(), RepresentationError> {
    let change = object_state::ChangeMsg::decode(msg)?;
    if let Some(change_type) = change.change_type {
        match change_type {
            change_msg::ChangeType::Add(object) | change_msg::ChangeType::Modify(object) => {
                let repr_opt = call_service(object).await?;
                if let Some(repr) = repr_opt {
                    produce::submit_representations(broker, topic, file, repr).await?;
                }
            }
            _ => (),
        }
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
    consume::start_consume_stream(broker, group, obj_topic, repr_topic).await;
    return Ok(());
}
