use futures::StreamExt;
use log::*;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer};
use rdkafka::message::Message;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Kafka error: {0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
    #[error("Cache error: {0}")]
    CacheError(#[from] crate::cache::DepError),
    #[error("String error: {0}")]
    StringError(#[from] std::str::Utf8Error),
    #[error("Message from partition {partition} and offset {offset} has no payload")]
    PayloadError { partition: i32, offset: i64 },
    #[error("Message from partition {partition} and offset {offset} has no file key set")]
    FileError { partition: i32, offset: i64 },
}

async fn handle_message<M: Message>(
    redis_conn: &mut redis::aio::MultiplexedConnection,
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
    crate::cache::update_deps(redis_conn, &file, offset, bytes).await?;
    Ok(())
}

async fn handle_stream(
    redis_conn: &mut redis::aio::MultiplexedConnection,
    brokers: &str,
    group_id: &str,
) -> Result<(), UpdateError> {
    let consumer: StreamConsumer<rdkafka::consumer::DefaultConsumerContext> = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create()?;

    consumer.subscribe(&["ObjectState"])?;

    // consumer.start() returns a stream. The stream can be used ot chain together expensive steps,
    // such as complex computations on a thread pool or asynchronous IO.
    let mut message_stream = consumer.start();

    while let Some(message) = message_stream.next().await {
        match message {
            Ok(m) => {
                if let Err(e) = handle_message(redis_conn, &m).await {
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

pub async fn update_cache(
    mut redis_conn: redis::aio::MultiplexedConnection,
    brokers: String,
    group_id: String,
) {
    if let Err(e) = handle_stream(&mut redis_conn, &brokers, &group_id).await {
        error!("{}", e);
    }
}
