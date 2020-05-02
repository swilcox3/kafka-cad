use super::*;
use prost::Message;
use rdkafka::config::ClientConfig;
//use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};

pub async fn submit_representations(brokers: &str, topic_name: &str, file: &str, msg: UpdateOutputMsg) -> Result<(), RepresentationError> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    // This loop is non blocking: all messages will be sent one after the other, without waiting
    // for the results.
    
    let mut payload = Vec::new();
    msg.encode(&mut payload)?;
    // The send operation on the topic returns a future, that will be completed once the
    // result or failure from Kafka will be received.
    producer
        .send(FutureRecord::to(topic_name).payload(&payload).key(file), 0).await.unwrap().unwrap();

    Ok(())
}
