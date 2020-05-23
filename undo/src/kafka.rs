use futures::StreamExt;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer};
use rdkafka::message::Message;
use thiserror::Error;
use tracing::*;
use tracing_futures::Instrument;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Kafka error: {0}")]
    KafkaError(#[from] rdkafka::error::KafkaError),
    #[error("Cache error: {0}")]
    CacheError(#[from] crate::UndoError),
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
    let resp = crate::cache::update_undo_cache(redis_conn, &file, offset, bytes)
        .instrument(info_span!("update_undo_cache"))
        .await;
    match resp {
        Ok(()) => Ok(()),
        Err(e) => Err(UpdateError::from(e)),
    }
}

async fn handle_stream(
    redis_url: &str,
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
        match crate::get_redis_conn(redis_url).await {
            Ok(mut redis_conn) => match message {
                Ok(m) => {
                    if let Err(e) = handle_message(&mut redis_conn, &m)
                        .instrument(info_span!("handle_message"))
                        .await
                    {
                        let span = info_span!("handle_message error");
                        let _enter = span.enter();
                        error!("{}", e);
                    }
                    let span = info_span!("commit_message");
                    let _enter = span.enter();
                    if let Err(e) = consumer.commit_message(&m, CommitMode::Async) {
                        error!("{}", e);
                    }
                }
                Err(e) => {
                    let span = info_span!("kafka message error");
                    let _enter = span.enter();
                    error!("{}", e);
                }
            },
            Err(e) => {
                let span = info_span!("redis connect error");
                let _enter = span.enter();
                error!("{}", e);
            }
        }
    }
    Ok(())
}

pub async fn update_cache(redis_url: String, brokers: String, group_id: String, topic: String) {
    std::thread::sleep(std::time::Duration::from_secs(30));
    if let Err(e) = handle_stream(&redis_url, &brokers, &group_id, &topic).await {
        error!("{}", e);
    }
}
