use anyhow::Result;
use dashmap::DashMap;
use futures_util::future::{select, Either};
use futures_util::{SinkExt, StreamExt};
use lazy_static::lazy_static;
use log::*;
use serde::Deserialize;
use tokio::sync::{mpsc, mpsc::Sender};
use tungstenite::Message;

mod kafka;

#[derive(Debug, Clone)]
pub struct UpdateMessage {
    file: String,
    msg: Vec<u8>,
}

pub struct ChannelSend {
    sender: Sender<UpdateMessage>,
    user: String,
}

lazy_static! {
    pub static ref FILE_TO_CHANNEL_MAP: DashMap<String, Vec<ChannelSend>> = DashMap::default();
}

#[derive(Deserialize)]
enum Commands {
    Subscribe { filename: String, user: String },
    Unsubscribe { filename: String, user: String },
}

async fn accept_connection(stream: tokio::net::TcpStream) {
    if let Err(e) = handle_connection(stream).await {
        error!("{:?}", e);
    }
}

async fn handle_connection(stream: tokio::net::TcpStream) -> Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    info!("New connection");
    let (mut ws_send, mut ws_rcv) = ws_stream.split();
    let (channel_send, mut channel_rcv) = mpsc::channel(100);
    let mut channel_fut = channel_rcv.next();
    let mut ws_fut = ws_rcv.next();
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
                                Ok(Commands::Subscribe { filename, user }) => {
                                    info!("New subscribe cmd for {:?}", filename);
                                    let send = ChannelSend {
                                        sender: channel_send.clone(),
                                        user: user,
                                    };
                                    match FILE_TO_CHANNEL_MAP.get_mut(&filename) {
                                        Some(mut entry) => {
                                            entry.value_mut().push(send);
                                        }
                                        None => {
                                            FILE_TO_CHANNEL_MAP.insert(filename, vec![send]);
                                        }
                                    }
                                }
                                Ok(Commands::Unsubscribe { filename, user }) => {
                                    info!("New unsubscribe cmd for {:?}", filename);
                                    if let Some(mut entry) = FILE_TO_CHANNEL_MAP.get_mut(&filename)
                                    {
                                        let senders = entry.value_mut();
                                        let mut index = 0usize;
                                        for send in senders.iter() {
                                            if send.user == user {
                                                senders.remove(index);
                                                break;
                                            }
                                            index += 1;
                                        }
                                    }
                                }
                                Err(e) => error!("Invalid JSON: {:?}", e),
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
                        debug!("Got message from channel, passing on");
                        ws_send.send(Message::Binary(ws_msg.msg)).await?;
                    }
                    None => {
                        debug!("Got None, breaking");
                        break;
                    }
                }
                ws_fut = ws_fut_continue;
                channel_fut = channel_rcv.next();
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
    tokio::spawn(kafka::consume(broker, group, topic));
    while let Ok((stream, _)) = server.accept().await {
        tokio::spawn(accept_connection(stream));
    }
    Ok(())
}
