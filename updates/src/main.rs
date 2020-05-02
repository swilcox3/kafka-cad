use futures_util::future::{select, Either};
use futures_util::{SinkExt, StreamExt};
use log::*;
use serde::Deserialize;
use tungstenite::Message;
use anyhow::Result;
use std::collections::HashSet;
use async_std::sync::Receiver;

mod kafka;

#[derive(Deserialize)]
enum Commands {
    Subscribe(String),
    Unsubscribe(String),
}

#[derive(Debug, Clone)]
pub struct UpdateMessage {
    file: String,
    msg: Vec<u8>
}

async fn accept_connection(kafka_rcv: Receiver<UpdateMessage>, stream: tokio::net::TcpStream) {
    if let Err(e) = handle_connection(kafka_rcv, stream).await {
        error!("{:?}", e);
    }
}

async fn handle_connection(mut kafka_rcv: Receiver<UpdateMessage>, stream: tokio::net::TcpStream) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    info!("New connection");
    let (mut ws_send, mut ws_rcv) = ws_stream.split();
    let mut channel_fut = kafka_rcv.next();
    let mut ws_fut = ws_rcv.next();
    let mut subs = HashSet::new();
    loop {
        trace!("Going to select");
        match select(ws_fut, channel_fut).await {
            Either::Left((ws_msg, channel_fut_continue)) => {
                trace!("Selected websocket");
                match ws_msg {
                    Some(msg) => {
                        let msg = msg?;
                        match msg {
                            Message::Ping(msg) => {
                                trace!("Ping");
                                ws_send.send(Message::Pong(msg)).await?;
                            }
                            Message::Pong(msg) => {
                                trace!("Pong");
                                ws_send.send(Message::Ping(msg)).await?;
                            }
                            Message::Close(_) => {
                                debug!("Close message received, breaking");
                                break;
                            }
                            Message::Text(sub_msg) => match serde_json::from_str(&sub_msg) {
                                Ok(Commands::Subscribe(filename)) => {
                                    info!("New subscribe cmd for {:?}", filename);
                                    subs.insert(filename);
                                }
                                Ok(Commands::Unsubscribe(filename)) => {
                                    info!("New unsubscribe cmd for {:?}", filename);
                                    subs.remove(&filename);
                                }
                                Err(e) => error!("Invalid JSON: {:?}", e)
                            },
                            _ => {
                                error!("Unexpected message {:?} received from client", msg);
                                ws_send
                                    .send(Message::Text(format!("Unexpected message {:?}", msg)))
                                    .await?
                            }
                        }
                    }
                    None => break,
                }
                channel_fut = channel_fut_continue;
                ws_fut = ws_rcv.next();
            }
            Either::Right((channel_msg, ws_fut_continue)) => {
                trace!("Selected channel");
                match channel_msg {
                    Some(ws_msg) => {
                        if subs.contains(&ws_msg.file) {
                            debug!("Got message from channel, passing on");
                            ws_send.send(Message::Binary(ws_msg.msg)).await?;
                        }
                    }
                    None => {
                        debug!("Got None, breaking");
                        break;
                    }
                }
                ws_fut = ws_fut_continue;
                channel_fut = kafka_rcv.next();
            }
        }
    }
    info!("Closing connection");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    env_logger::init();
    let run_url = std::env::var("RUN_URL").unwrap();
    let broker = std::env::var("BROKER").unwrap();
    let group = std::env::var("GROUP").unwrap();
    let topic = std::env::var("TOPIC").unwrap();
    let mut server = tokio::net::TcpListener::bind(&run_url).await.unwrap();
    info!("Listening for updates");
    let (channel_send, channel_rcv) = async_std::sync::channel(100);
    tokio::spawn(kafka::consume(channel_send, broker, group, topic));
    while let Ok((stream, _)) = server.accept().await {
        tokio::spawn(accept_connection(channel_rcv.clone(), stream));
    }
    Ok(())
}