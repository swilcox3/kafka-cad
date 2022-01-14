use crate::*;
use log::*;
use prost::Message;
use url::Url;

pub type Socket = tungstenite::protocol::WebSocket<tungstenite::client::AutoStream>;

pub fn connect(url: &str) -> Result<Socket> {
    let (socket, response) = tungstenite::connect(Url::parse(url)?)?;

    debug!("Connected to {:?}", url);
    debug!("Response HTTP code: {}", response.status());
    if response.status() == http::StatusCode::OK {
        Ok(socket)
    } else {
        Err(anyhow!("Failed to connect"))
    }
}

pub fn read_message(socket: &mut Socket) -> Result<UpdateChangeMsg> {
    let msg = socket.read_message()?;
    if let tungstenite::Message::Binary(inner) = msg {
        let change_msg = UpdateChangeMsg::decode(inner.as_slice())?;
        Ok(change_msg)
    } else {
        error!("Didn't get binary msg");
        Err(anyhow!("Incorrect message recieved"))
    }
}
