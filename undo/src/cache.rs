//! The object cache for a file maps objIDs to a list of the last X number of changes to that object.
//! X is configurable.

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

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum UndoChangeType {
    Add,
    Modify,
    Delete,
    NotSet,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
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
    debug!("adding undo entry {:?}", entry);
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
    trace!("Undo stack: {:?}", undo_stack);
    let event: Option<String> = redis_conn.rpop(undo_stack).await?;
    trace!("popped event: {:?}", event);
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

async fn update_undo_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: String,
    offset: i64,
    obj_id: String,
    change_type: UndoChangeType,
) -> Result<(), UndoError> {
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
    update_undo_event(redis_conn, file, user, offset, obj_id, change_type).await?;
    Ok(())
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
    trace!("made it event {:?}", event);
    push_redo_event(redis_conn, file, user, &event).await?;
    trace!("Pushed redo event");
    let list = get_undo_event_list(redis_conn, &event).await?;
    debug!("got list: {:?}", list);
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    pub async fn test_get_conn() -> MultiplexedConnection {
        let env_opt = std::env::var("REDIS_URL");
        let redis_url = if let Ok(url) = env_opt {
            url
        } else {
            String::from("redis://127.0.0.1:6379")
        };
        let client = redis::Client::open(redis_url).unwrap();
        let (conn, fut) = client.get_multiplexed_async_connection().await.unwrap();
        tokio::spawn(fut);
        conn
    }

    #[tokio_macros::test]
    async fn test_cache() {
        let mut conn = test_get_conn().await;
        let file = Uuid::new_v4().to_string();
        let user = Uuid::new_v4().to_string();
        let obj_1 = Uuid::new_v4().to_string();
        begin_undo_event(&mut conn, &file, &user).await.unwrap();
        let offset = 1;
        update_undo_event(
            &mut conn,
            &file,
            user.clone(),
            offset,
            obj_1.clone(),
            UndoChangeType::Add,
        )
        .await
        .unwrap();
        let event = undo(&mut conn, &file, &user).await.unwrap();
        assert_eq!(event.len(), 1);
        let answer = UndoEntry {
            offset,
            obj_id: obj_1,
            change_type: UndoChangeType::Add,
        };
        assert_eq!(event[0], answer);
    }
}
