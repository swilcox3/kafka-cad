use crate::*;
use futures::StreamExt;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer};
use rdkafka::message::Message;

async fn handle_message<M: Message>(
    broker: &str,
    repr_topic: &str,
    ops_url: String,
    m: &M,
) -> Result<(), RepresentationError> {
    let partition = m.partition();
    let offset = m.offset();
    let bytes = m
        .payload()
        .ok_or(RepresentationError::PayloadError { partition, offset })?;
    let file_bytes = m
        .key()
        .ok_or(RepresentationError::FileError { partition, offset })?;
    let file = std::str::from_utf8(file_bytes)?;
    calc_representation(broker, repr_topic, file, ops_url, bytes).await?;
    Ok(())
}

async fn handle_stream(
    brokers: &str,
    group_id: &str,
    obj_topic: &str,
    repr_topic: &str,
    ops_url: String,
) -> Result<(), RepresentationError> {
    let consumer: StreamConsumer<rdkafka::consumer::DefaultConsumerContext> = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create()?;

    consumer.subscribe(&[obj_topic])?;
    println!("Subscribed to {:?}", obj_topic);

    // consumer.start() returns a stream. The stream can be used ot chain together expensive steps,
    // such as complex computations on a thread pool or asynchronous IO.
    let mut message_stream = consumer.start();

    while let Some(message) = message_stream.next().await {
        match message {
            Ok(m) => {
                if let Err(e) = handle_message(brokers, repr_topic, ops_url.clone(), &m).await {
                    println!("{}", e);
                }
                if let Err(e) = consumer.commit_message(&m, CommitMode::Async) {
                    println!("{}", e);
                }
            }
            Err(e) => {
                println!("{}", e);
            }
        }
    }
    Ok(())
}

pub async fn start_consume_stream(
    brokers: String,
    group_id: String,
    obj_topic: String,
    repr_topic: String,
    ops_url: String,
) {
    std::thread::sleep(std::time::Duration::from_secs(30));
    println!("Start consuming stream on topic {:?}", obj_topic);
    if let Err(e) = handle_stream(&brokers, &group_id, &obj_topic, &repr_topic, ops_url).await {
        println!("{}", e);
    }
}
