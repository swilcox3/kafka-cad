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

async fn add_entry_to_event(
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

async fn get_event_entries(
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

async fn push_event_to_stack(
    redis_conn: &mut MultiplexedConnection,
    stack: &str,
    event: &str,
) -> Result<(), UndoError> {
    redis_conn.rpush(stack, event).await?;
    Ok(())
}

async fn delete_event_if_empty(
    redis_conn: &mut MultiplexedConnection,
    stack: &str,
    event: &str,
) -> Result<(), UndoError> {
    let event_len: u64 = redis_conn.llen(event).await?;
    if event_len == 0 {
        redis_conn.lrem(stack, 1, event).await?;
    }
    Ok(())
}

async fn pop_entry_from_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    event: &str,
    obj_id: &str,
) -> Result<UndoEntry, UndoError> {
    let entries = get_event_entries(redis_conn, &event).await?;
    let mut result = None;
    for entry in entries {
        if entry.obj_id == obj_id {
            //We have to do this because Redis doesn't have a remove by index
            let serialized = bincode::serialize(&entry)?;
            redis_conn.lrem(event, 1, serialized).await?;
            result = Some(entry);
            break;
        }
    }
    match result {
        Some(entry) => Ok(entry),
        None => Err(UndoError::NoObjInUndoEvent(
            String::from(obj_id),
            String::from(event),
            String::from(file),
        )),
    }
}

async fn get_current_event_in_stack(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
    stack: &str,
) -> Result<String, UndoError> {
    let cur_event: Option<String> = redis_conn.lindex(stack, -1).await?;
    match cur_event {
        Some(event) => Ok(event),
        None => Err(UndoError::NoUndoEvent(
            String::from(user),
            String::from(file),
        )),
    }
}

async fn update_event_in_stack(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
    stack: &str,
    offset: i64,
    obj_id: String,
    change_type: UndoChangeType,
) -> Result<(), UndoError> {
    let event = get_current_event_in_stack(redis_conn, file, user, stack).await?;
    add_entry_to_event(redis_conn, &event, obj_id, offset, change_type).await?;
    Ok(())
}

async fn move_entry_between_stacks(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
    from_stack: &str,
    to_stack: &str,
    from_event: &str,
    obj_id: String,
    new_offset: i64,
    new_change_type: UndoChangeType,
) -> Result<(), UndoError> {
    pop_entry_from_event(redis_conn, file, from_event, &obj_id).await?;
    delete_event_if_empty(redis_conn, from_stack, from_event).await?;
    update_event_in_stack(
        redis_conn,
        file,
        user,
        to_stack,
        new_offset,
        obj_id,
        new_change_type,
    )
    .await?;
    Ok(())
}

async fn update_undo_cache_inner(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    msg: ChangeMsg,
) -> Result<(), UndoError> {
    let user = msg.user;
    let (obj_id, change_type) = match msg.change_type {
        Some(change_msg::ChangeType::Add(inner_msg)) => (inner_msg.id, UndoChangeType::Add),
        Some(change_msg::ChangeType::Modify(inner_msg)) => (inner_msg.id, UndoChangeType::Modify),
        Some(change_msg::ChangeType::Delete(inner_msg)) => (inner_msg.id, UndoChangeType::Delete),
        None => (String::new(), UndoChangeType::NotSet),
    };
    let undo_stack = undo_stack(file, &user);
    let redo_stack = redo_stack(file, &user);
    match msg.change_source {
        Some(change_msg::ChangeSource::UserAction(..)) => {
            update_event_in_stack(
                redis_conn,
                file,
                &user,
                &undo_stack,
                offset,
                obj_id,
                change_type,
            )
            .await?;
        }
        Some(change_msg::ChangeSource::Undo(event)) => {
            move_entry_between_stacks(
                redis_conn,
                file,
                &user,
                &undo_stack,
                &redo_stack,
                &event,
                obj_id,
                offset,
                change_type,
            )
            .await?;
        }
        Some(change_msg::ChangeSource::Redo(event)) => {
            move_entry_between_stacks(
                redis_conn,
                file,
                &user,
                &redo_stack,
                &undo_stack,
                &event,
                obj_id,
                offset,
                change_type,
            )
            .await?;
        }
        None => (),
    }
    Ok(())
}

pub async fn update_undo_cache(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    msg_bytes: &[u8],
) -> Result<(), UndoError> {
    let msg = ChangeMsg::decode(msg_bytes)?;
    info!("Got msg: {:?}", msg);
    update_undo_cache_inner(redis_conn, file, offset, msg).await?;
    Ok(())
}

async fn begin_event(redis_conn: &mut MultiplexedConnection, stack: &str) -> Result<(), UndoError> {
    let event = Uuid::new_v4().to_string();
    push_event_to_stack(redis_conn, stack, &event).await?;
    Ok(())
}

pub async fn begin_undo_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<(), UndoError> {
    let undo_stack = undo_stack(file, user);
    begin_event(redis_conn, &undo_stack).await?;
    Ok(())
}

async fn begin_redo_event(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<(), UndoError> {
    let redo_stack = redo_stack(file, user);
    begin_event(redis_conn, &redo_stack).await?;
    Ok(())
}

async fn get_current_event_and_list(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
    stack: &str,
) -> Result<(String, Vec<UndoEntry>), UndoError> {
    let event = get_current_event_in_stack(redis_conn, file, user, stack).await?;
    let list = get_event_entries(redis_conn, &event).await?;
    debug!("got list: {:?}", list);
    Ok((event, list))
}

pub async fn undo(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<(String, Vec<UndoEntry>), UndoError> {
    let undo_stack = undo_stack(file, user);
    let results = get_current_event_and_list(redis_conn, file, user, &undo_stack).await?;
    begin_redo_event(redis_conn, file, user).await?;
    Ok(results)
}

pub async fn redo(
    redis_conn: &mut MultiplexedConnection,
    file: &str,
    user: &str,
) -> Result<(String, Vec<UndoEntry>), UndoError> {
    let redo_stack = redo_stack(file, user);
    let results = get_current_event_and_list(redis_conn, file, user, &redo_stack).await?;
    begin_undo_event(redis_conn, file, user).await?;
    Ok(results)
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
        let msg = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                id: obj_1.clone(),
                dependencies: None,
                obj_data: Vec::new(),
            })),
            change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
        };
        update_undo_cache_inner(&mut conn, &file, offset, msg)
            .await
            .unwrap();

        let (event, list) = undo(&mut conn, &file, &user).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].obj_id, obj_1);
        assert_eq!(list[0].offset, offset);
        assert_eq!(list[0].change_type, UndoChangeType::Add);

        let offset = 2;
        let undo_msg = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Delete(DeleteMsg {
                id: obj_1.clone(),
            })),
            change_source: Some(change_msg::ChangeSource::Undo(event)),
        };
        update_undo_cache_inner(&mut conn, &file, offset, undo_msg)
            .await
            .unwrap();

        //Undo again, there shouldn't be an undo event anymore so this should throw an error
        assert!(undo(&mut conn, &file, &user).await.is_err());

        let (event, list) = redo(&mut conn, &file, &user).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].obj_id, obj_1);
        assert_eq!(list[0].offset, offset);
        assert_eq!(list[0].change_type, UndoChangeType::Delete);

        let offset = 3;
        let redo_msg = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                id: obj_1.clone(),
                dependencies: None,
                obj_data: Vec::new(),
            })),
            change_source: Some(change_msg::ChangeSource::Redo(event)),
        };
        update_undo_cache_inner(&mut conn, &file, offset, redo_msg)
            .await
            .unwrap();

        //Redo again, there shouldn't be a redo event anymore so this should throw an error
        assert!(redo(&mut conn, &file, &user).await.is_err());

        //Now undo/redo again
        let (event, list) = undo(&mut conn, &file, &user).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].obj_id, obj_1);
        assert_eq!(list[0].offset, offset);
        assert_eq!(list[0].change_type, UndoChangeType::Add);
        let offset = 4;
        let undo_msg = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Delete(DeleteMsg {
                id: obj_1.clone(),
            })),
            change_source: Some(change_msg::ChangeSource::Undo(event)),
        };
        update_undo_cache_inner(&mut conn, &file, offset, undo_msg)
            .await
            .unwrap();
        let (event, list) = redo(&mut conn, &file, &user).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].obj_id, obj_1);
        assert_eq!(list[0].offset, offset);
        assert_eq!(list[0].change_type, UndoChangeType::Delete);
        let offset = 5;
        let redo_msg = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                id: obj_1.clone(),
                dependencies: None,
                obj_data: Vec::new(),
            })),
            change_source: Some(change_msg::ChangeSource::Redo(event)),
        };
        update_undo_cache_inner(&mut conn, &file, offset, redo_msg)
            .await
            .unwrap();
    }
}
