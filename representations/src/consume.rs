use futures::StreamExt;
use log::*;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer};
use rdkafka::message::Message;
use crate::*;

async fn handle_message<M: Message>(
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
    Ok(())
}

async fn handle_stream(
    brokers: &str,
    group_id: &str,
    topic: &str,
) -> Result<(), RepresentationError> {
    let consumer: StreamConsumer<rdkafka::consumer::DefaultConsumerContext> = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create()?;

    consumer.subscribe(&[topic])?;

    // consumer.start() returns a stream. The stream can be used ot chain together expensive steps,
    // such as complex computations on a thread pool or asynchronous IO.
    let mut message_stream = consumer.start();

    while let Some(message) = message_stream.next().await {
        match message {
            Ok(m) => {
                if let Err(e) = handle_message(&m).await {
                    error!("{}", e);
                }
                if let Err(e) = consumer.commit_message(&m, CommitMode::Async) {
                    error!("{}", e);
                }
            }
            Err(e) => {
                error!("{}", e);
            }
        }
    }
    Ok(())
}

pub async fn start_consume_stream(
    brokers: String,
    group_id: String,
    topic: String,
) {
    std::thread::sleep(std::time::Duration::from_secs(30));
    if let Err(e) = handle_stream(&brokers, &group_id, &topic).await {
        error!("{}", e);
    }
}
