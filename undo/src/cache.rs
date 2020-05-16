//! The object cache for a file maps objIDs to a list of the last X number of changes to that object.
//! X is configurable.

use log::*;
use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::*;

fn undo_stack(file: &str, user: &str) -> String {
    format!("{}:{}:undo", file, user)
}

fn redo_stack(file: &str, user: &str) -> String {
    format!("{}:{}:redo", file, user)
}

#[derive(Debug, Serialize, Deserialize)]
pub enum UndoChangeType {
    Add,
    Modify,
    Delete,
    NotSet,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UndoEntry {
    pub offset: i64,
    pub obj_id: String,
    pub change_type: UndoChangeType,
}

async fn add_undo_entry(
    redis_conn: &mut MultiplexedConnection,
    event: &str,
    obj_id: String,
    offset: i64,
    change_type: UndoChangeType,
) -> Result<(), UndoError> {
    let entry = UndoEntry {
        offset,
        obj_id,
        change_type,
    };
    let serialized = bincode::serialize(&entry)?;
    redis_conn.rpush(event, serialized).await?;
    Ok(())
}

async fn get_undo_event_list(
    redis_conn: &mut MultiplexedConnection,
    event: &str,
) -> Result<Vec<UndoEntry>, UndoError> {
    let serialized: Vec<Vec<u8>> = redis_conn.lrange(event, 0, -1).await?;
    let mut results = Vec::new();
    for entry in serialized {
        results.push(bincode::deserialize(&entry)?);
    }
    Ok(results)
}

async fn push_undo_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
    event: &str,
) -> Result<(), UndoError> {
    let undo_stack = undo_stack(file, user);
    debug!("Pushing to undo stack: {:?}", undo_stack);
    redis_conn.rpush(undo_stack, event).await?;
    Ok(())
}

async fn pop_undo_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<String, UndoError> {
    let undo_stack = undo_stack(file, user);
    let event: Option<String> = redis_conn.rpop(undo_stack).await?;
    match event {
        Some(event) => Ok(event),
        None => Err(UndoError::NoUndoEvent(
            String::from(user),
            String::from(file),
        )),
    }
}

async fn push_redo_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
    event: &str,
) -> Result<(), UndoError> {
    let redo_stack = redo_stack(file, user);
    redis_conn.rpush(redo_stack, event).await?;
    Ok(())
}

async fn pop_redo_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<String, UndoError> {
    let redo_stack = redo_stack(file, user);
    let event: Option<String> = redis_conn.rpop(redo_stack).await?;
    match event {
        Some(event) => Ok(event),
        None => Err(UndoError::NoUndoEvent(
            String::from(user),
            String::from(file),
        )),
    }
}

pub async fn update_undo_cache(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    msg_bytes: &[u8],
) -> Result<(), UndoError> {
    let msg = ChangeMsg::decode(msg_bytes)?;
    info!("Got msg: {:?}", msg);
    let user = msg.user;
    let (obj_id, change_type) = match msg.change_type {
        Some(change_msg::ChangeType::Add(msg)) => (msg.id, UndoChangeType::Add),
        Some(change_msg::ChangeType::Modify(msg)) => (msg.id, UndoChangeType::Modify),
        Some(change_msg::ChangeType::Delete(msg)) => (msg.id, UndoChangeType::Delete),
        None => (String::new(), UndoChangeType::NotSet),
    };
    let undo_stack = undo_stack(file, &user);
    info!("Updating undo stack: {:?}", undo_stack);
    let cur_event: Option<String> = redis_conn.lindex(undo_stack, -1).await?;
    match cur_event {
        Some(event) => {
            add_undo_entry(redis_conn, &event, obj_id, offset, change_type).await?;
            Ok(())
        }
        None => Err(UndoError::NoUndoEvent(user, String::from(file))),
    }
}

pub async fn begin_undo_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<(), UndoError> {
    let event = Uuid::new_v4().to_string();
    push_undo_event(redis_conn, file, user, &event).await?;
    Ok(())
}

pub async fn undo(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<Vec<UndoEntry>, UndoError> {
    let event = pop_undo_event(redis_conn, file, user).await?;
    push_redo_event(redis_conn, file, user, &event).await?;
    let list = get_undo_event_list(redis_conn, &event).await?;
    Ok(list)
}

pub async fn redo(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<Vec<UndoEntry>, UndoError> {
    let event = pop_redo_event(redis_conn, file, user).await?;
    push_undo_event(redis_conn, file, user, &event).await?;
    let list = get_undo_event_list(redis_conn, &event).await?;
    Ok(list)
}
