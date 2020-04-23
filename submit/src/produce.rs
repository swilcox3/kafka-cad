use super::*;
use futures::FutureExt;
use prost::Message;
use rdkafka::config::ClientConfig;
//use rdkafka::message::Message;
use rdkafka::producer::{FutureProducer, FutureRecord};

pub async fn submit_changes(brokers: &str, topic_name: &str, payloads: Vec<ChangeMsg>) -> Vec<i64> {
    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("Producer creation error");

    // This loop is non blocking: all messages will be sent one after the other, without waiting
    // for the results.
    let futures = payloads
        .into_iter()
        .map(|msg| {
            let mut payload = Vec::new();
            msg.encode(&mut payload).unwrap();
            let key = msg.id;
            // The send operation on the topic returns a future, that will be completed once the
            // result or failure from Kafka will be received.
            producer
                .send(FutureRecord::to(topic_name).payload(&payload).key(&key), 0)
                .map(move |delivery_status| {
                    debug!("Delivery status for message {} received", key);
                    delivery_status
                })
        })
        .collect::<Vec<_>>();

    // This loop will wait until all delivery statuses have been received received.
    let mut offsets = Vec::new();
    for future in futures {
        let (_, offset) = future.await.unwrap().unwrap();
        offsets.push(offset);
    }
    offsets
}
