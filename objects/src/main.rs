use futures::StreamExt;
use log::*;
use rdkafka::config::{ClientConfig, RDKafkaLogLevel};
use rdkafka::consumer::stream_consumer::StreamConsumer;
use rdkafka::consumer::{CommitMode, Consumer};
use rdkafka::message::Message;
use tonic::transport::Server;
use tonic::{Request, Response, Status};

mod cache;
use cache::*;

struct ObjectService {
    redis_conn: redis::aio::MultiplexedConnection,
}

#[tonic::async_trait]
impl objects_server::Objects for ObjectService {
    async fn get_objects(
        &self,
        request: Request<GetObjectsInput>,
    ) -> Result<Response<GetObjectsOutput>, Status> {
        let msg = request.get_ref();
        info!("Get objects: {:?}", msg);
        let mut redis_conn = self.redis_conn.clone();
        let objects = cache::get_objects(&mut redis_conn, msg)
            .await
            .map_err(to_status)?;
        Ok(Response::new(GetObjectsOutput { objects }))
    }
}

async fn update_cache(
    mut redis_conn: redis::aio::MultiplexedConnection,
    brokers: &str,
    group_id: &str,
    file: &str,
    max_cache_len: u64,
) {
    let consumer: StreamConsumer<rdkafka::consumer::DefaultConsumerContext> = ClientConfig::new()
        .set("group.id", group_id)
        .set("bootstrap.servers", brokers)
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set_log_level(RDKafkaLogLevel::Debug)
        .create()
        .expect("Consumer creation failed");

    consumer
        .subscribe(&["ObjectState"])
        .expect("Can't subscribe to specified topics");

    // consumer.start() returns a stream. The stream can be used ot chain together expensive steps,
    // such as complex computations on a thread pool or asynchronous IO.
    let mut message_stream = consumer.start();

    while let Some(message) = message_stream.next().await {
        match message {
            Ok(m) => match m.payload() {
                Some(bytes) => {
                    let offset = m.offset();
                    cache::update_object_cache(
                        &mut redis_conn,
                        file,
                        offset as u64,
                        bytes,
                        max_cache_len,
                    )
                    .await
                    .unwrap();
                    consumer.commit_message(&m, CommitMode::Async).unwrap();
                }
                None => error!("No payload"),
            },
            Err(e) => error!("Kafka error: {}", e),
        };
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let addr = "0.0.0.0:6000".parse().unwrap();

    let redis_url = std::env::var("REDIS_URL").unwrap();
    info!("redis_url: {:?}", redis_url);
    let client = redis::Client::open(redis_url).unwrap();
    let (redis_conn, fut) = client.get_multiplexed_async_connection().await.unwrap();
    tokio::spawn(fut);

    let svc = objects_server::ObjectsServer::new(ObjectService { redis_conn });

    info!("Running on {:?}", addr);
    Server::builder()
        .add_service(svc)
        .serve(addr)
        .await
        .unwrap();
    Ok(())
}
