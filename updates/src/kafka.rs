use futures::StreamExt;
use log::*;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer};
use rdkafka::message::Message;
use thiserror::Error;
use async_std::sync::Sender;
use super::*;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Kafka error: {0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
    #[error("String error: {0}")]
    StringError(#[from] std::str::Utf8Error),
    #[error("Message from partition {partition} and offset {offset} has no payload")]
    PayloadError { partition: i32, offset: i64 },
    #[error("Message from partition {partition} and offset {offset} has no file key set")]
    FileError { partition: i32, offset: i64 },
}

async fn handle_message<M: Message>(
    sender: &mut Sender<UpdateMessage>,
    m: &M,
) -> Result<(), UpdateError> {
    let partition = m.partition();
    let offset = m.offset();
    let bytes = m
        .payload()
        .ok_or(UpdateError::PayloadError { partition, offset })?;
    let file_bytes = m
        .key()
        .ok_or(UpdateError::FileError { partition, offset })?;
    let file = std::str::from_utf8(file_bytes)?;
    sender.send(UpdateMessage{
        file: String::from(file),
        msg: bytes.to_vec()
    }).await;
    Ok(())
}

async fn handle_stream(
    sender: &mut Sender<UpdateMessage>,
    brokers: &str,
    group_id: &str,
    topic: &str,
) -> Result<(), UpdateError> {
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
                if let Err(e) = handle_message(sender, &m).await {
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

pub async fn consume(
    mut sender: Sender<UpdateMessage>,
    brokers: String,
    group_id: String,
    topic: String,
) {
    std::thread::sleep(std::time::Duration::from_secs(30));
    if let Err(e) = handle_stream(&mut sender, &brokers, &group_id, &topic).await {
        error!("{}", e);
    }
}
