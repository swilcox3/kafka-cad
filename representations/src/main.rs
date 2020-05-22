use tracing::*;
use trace_lib::*;
use prost::Message;
use thiserror::Error;

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
    #[error("No change type set")]
    NoChangeType,
}

async fn call_service(
    ops_url: String,
    object: ChangeMsg,
    span: &Span
) -> Result<Option<UpdateOutputMsg>, RepresentationError> {
    let mut client = operations::operations_client::OperationsClient::connect(ops_url).await?;
    let resp = client
        .client_representation(TracedRequest::new(ClientRepresentationInput {
            objects: vec![object],
        }, span))
        .await;
    let mut representation = trace_response(resp)?;
    Ok(representation.outputs.pop())
}

pub async fn calc_representation(
    broker: &str,
    topic: &str,
    file: &str,
    ops_url: String,
    msg: &[u8],
) -> Result<(), RepresentationError> {
    let span = info_span!("calc_representation");
    let _enter = span.enter();
    let change = object_state::ChangeMsg::decode(msg)?;
    debug!("Got change: {:?}", change);
    let obj_id = match &change.change_type {
        Some(change_msg::ChangeType::Add(object))
        | Some(change_msg::ChangeType::Modify(object)) => object.id.clone(),
        Some(change_msg::ChangeType::Delete(msg)) => msg.id.clone(),
        None => return Err(RepresentationError::NoChangeType),
    };
    let user = change.user.clone();
    let repr_opt = call_service(ops_url, change, &span).await?;
    info!("Got representation: {:?}", repr_opt);
    if let Some(repr) = repr_opt {
        let update_change = UpdateChangeMsg {
            file: String::from(file),
            user: user,
            obj_id,
            update: Some(repr),
        };
        produce::submit_representations(broker, topic, file, update_change).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let broker = std::env::var("BROKER").unwrap();
    let jaeger_url = std::env::var("JAEGER_URL").unwrap();
    let group = std::env::var("GROUP").unwrap();
    let obj_topic = std::env::var("OBJ_TOPIC").unwrap();
    let repr_topic = std::env::var("REPR_TOPIC").unwrap();
    let ops_url = std::env::var("OPS_URL").unwrap();
    init_tracer(&jaeger_url, "representations")?;
    consume::start_consume_stream(broker, group, obj_topic, repr_topic, ops_url).await;
    return Ok(());
}
