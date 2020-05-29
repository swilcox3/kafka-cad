//! The object cache for a file maps objIDs to a list of the last X number of changes to that object.
//! X is configurable.  

use async_stream::try_stream;
use futures::stream::Stream;
use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::*;

mod geom {
    tonic::include_proto!("geom");
}

mod object_state {
    tonic::include_proto!("object_state");
}

pub mod objects {
    tonic::include_proto!("objects");
}

pub use object_state::*;
pub use objects::*;

#[derive(Debug, Error)]
pub enum ObjError {
    #[error("Object {0} not found")]
    ObjNotFound(String),
    #[error("Prost encode error: {0}")]
    ProstEncodeError(#[from] prost::EncodeError),
    #[error("Prost decode error: {0}")]
    ProstDecodeError(#[from] prost::DecodeError),
    #[error("Redis error: {0:?}")]
    DatabaseError(#[from] redis::RedisError),
    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
}

impl Into<tonic::Status> for ObjError {
    fn into(self) -> tonic::Status {
        let msg = format!("{}", self);
        let code = match self {
            ObjError::DatabaseError(..)
            | ObjError::BincodeError(..)
            | ObjError::ProstEncodeError(..)
            | ObjError::ProstDecodeError(..) => tonic::Code::Internal,
            ObjError::ObjNotFound(..) => tonic::Code::NotFound,
        };
        tonic::Status::new(code, msg)
    }
}

pub fn to_status<T: Into<ObjError>>(err: T) -> tonic::Status {
    let obj_error: ObjError = err.into();
    obj_error.into()
}

fn obj_cache(file: &str, key: &str) -> String {
    format!("{}:{}", file, key)
}

fn file_offset(file: &str) -> String {
    format!("{}:offset", file)
}

fn latest_obj_list(file: &str) -> String {
    format!("{}:obj_list", file)
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct ObjEntry {
    offset: i64,
    object: Vec<u8>,
}

async fn store_object_change(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    key: &str,
    obj: &[u8],
) -> Result<(), ObjError> {
    let obj_cache = obj_cache(file, key);
    trace!(
        "Pushing obj at offset {} to obj_cache {:?}",
        offset,
        obj_cache
    );
    let entry = ObjEntry {
        offset,
        object: Vec::from(obj),
    };
    let serialized = bincode::serialize(&entry)?;
    conn.lpush(&obj_cache, serialized).await?;
    Ok(())
}

async fn store_file_offset(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
) -> Result<(), ObjError> {
    let file_offset = file_offset(file);
    trace!("Setting file {} to offset {}", file, offset);
    conn.set(file_offset, offset).await?;
    Ok(())
}

async fn update_latest_obj_list(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj: &ChangeMsg,
) -> Result<(), ObjError> {
    let latest_list = latest_obj_list(file);
    match &obj.change_type {
        Some(change_msg::ChangeType::Add(object)) => {
            conn.sadd(latest_list, &object.id).await?;
        }
        Some(change_msg::ChangeType::Delete(msg)) => {
            conn.srem(latest_list, &msg.id).await?;
        }
        Some(change_msg::ChangeType::Modify(..)) | None => (),
    }
    Ok(())
}

pub fn get_latest_obj_list(
    mut conn: MultiplexedConnection,
    file: String,
) -> impl Stream<Item = Result<String, ObjError>> {
    try_stream! {
        let latest_list = latest_obj_list(&file);
        let mut ids: Vec<String> = conn.smembers(latest_list).await?;
        for id in ids {
            yield id;
        }
    }
}

async fn get_object(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    key: &str,
) -> Result<Vec<u8>, ObjError> {
    trace!(
        "getting object {:?} in file {:?} at offset {}",
        key,
        file,
        offset
    );
    let obj_cache = obj_cache(file, key);
    let cache_length: isize = conn.llen(&obj_cache).await?;
    debug!("Cache length: {:?}", cache_length);
    if cache_length == 0 {
        return Err(ObjError::ObjNotFound(String::from(key)));
    }
    for i in 0isize..cache_length {
        let serialized: Vec<u8> = conn.lindex(&obj_cache, i).await?;
        let entry: ObjEntry = bincode::deserialize(&serialized)?;
        trace!(
            "Comparing cache_offset {:?} to offset {:?}",
            entry.offset,
            offset
        );
        if entry.offset <= offset {
            return Ok(entry.object);
        }
    }
    Err(ObjError::ObjNotFound(String::from(key)))
}

pub async fn update_object_cache(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    input: &[u8],
) -> Result<(), ObjError> {
    let object = object_state::ChangeMsg::decode(input)?;
    info!("Updating object cache: {:?}", object);
    let id = match &object.change_type {
        Some(change_msg::ChangeType::Add(object))
        | Some(change_msg::ChangeType::Modify(object)) => object.id.clone(),
        Some(change_msg::ChangeType::Delete(msg)) => msg.id.clone(),
        None => {
            return Err(ObjError::ObjNotFound(String::from(
                "No change type specified",
            )));
        }
    };
    store_object_change(conn, file, offset, &id, input).await?;
    store_file_offset(conn, file, offset).await?;
    update_latest_obj_list(conn, file, &object).await?;
    Ok(())
}

pub async fn get_objects(
    conn: &mut MultiplexedConnection,
    input: &GetObjectsInput,
) -> Result<Vec<OptionChangeMsg>, ObjError> {
    debug!("get_objects input: {:?}", input);
    let mut results = Vec::new();
    for entry in &input.obj_ids {
        let mut current = OptionChangeMsg { change: None };
        match get_object(conn, &input.file, entry.offset, &entry.obj_id).await {
            Ok(bytes) => {
                current.change = Some(ChangeMsg::decode(bytes.as_ref())?);
            }
            Err(e) => match e {
                ObjError::ObjNotFound(..) => {
                    info!("Object {:?} not found", entry);
                }
                _ => {
                    error!("{}", e);
                    return Err(e);
                }
            },
        }
        results.push(current);
    }
    info!("Found objects: {:?}", results);
    Ok(results)
}

pub async fn get_latest_offset(
    conn: &mut MultiplexedConnection,
    input: &GetLatestOffsetInput,
) -> Result<i64, ObjError> {
    let file_offset = file_offset(&input.file);
    let offset: i64 = conn.get(file_offset).await?;
    Ok(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use uuid::Uuid;

    pub async fn test_get_conn() -> MultiplexedConnection {
        let _ = env_logger::Builder::new()
            .filter_module("objects", log::LevelFilter::Trace)
            .is_test(true)
            .try_init();
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
        let id = Uuid::new_v4().to_string();
        let file = Uuid::new_v4().to_string();
        let user = Uuid::new_v4().to_string();
        let change_1 = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                id: id.clone(),
                dependencies: None,
                obj_data: vec![],
            })),
            change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
        };
        let mut change_1_bytes = Vec::new();
        change_1.encode(&mut change_1_bytes).unwrap();
        let offset_1 = 4;

        //Nothing in cache yet, this should error out
        assert!(get_object(&mut conn, &file, offset_1, &id).await.is_err());

        update_object_cache(&mut conn, &file, offset_1, &change_1_bytes)
            .await
            .unwrap();
        assert_eq!(
            get_object(&mut conn, &file, offset_1, &id).await.unwrap(),
            change_1_bytes
        );

        let change_2 = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Modify(ObjectMsg {
                id: id.clone(),
                dependencies: None,
                obj_data: String::from("modified").into_bytes(),
            })),
            change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
        };
        let mut change_2_bytes = Vec::new();
        change_2.encode(&mut change_2_bytes).unwrap();
        let offset_2 = 5;
        update_object_cache(&mut conn, &file, offset_2, &change_2_bytes)
            .await
            .unwrap();
        assert_eq!(
            get_object(&mut conn, &file, offset_2, &id).await.unwrap(),
            change_2_bytes
        );

        let change_3 = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Delete(DeleteMsg { id: id.clone() })),
            change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
        };
        let mut change_3_bytes = Vec::new();
        change_3.encode(&mut change_3_bytes).unwrap();
        let offset_3 = 6;
        update_object_cache(&mut conn, &file, offset_3, &change_3_bytes)
            .await
            .unwrap();
        assert_eq!(
            get_object(&mut conn, &file, offset_3, &id).await.unwrap(),
            change_3_bytes
        );

        assert_eq!(
            get_object(&mut conn, &file, offset_1, &id).await.unwrap(),
            change_1_bytes
        );

        assert!(get_object(&mut conn, &file, offset_1 - 1, &id)
            .await
            .is_err());
    }

    #[tokio_macros::test]
    async fn test_get_latest_list() {
        let mut conn = test_get_conn().await;
        let id_1 = Uuid::new_v4().to_string();
        let id_2 = Uuid::new_v4().to_string();
        let id_3 = Uuid::new_v4().to_string();
        let file = Uuid::new_v4().to_string();
        let user = Uuid::new_v4().to_string();
        let change_1 = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                id: id_1.clone(),
                dependencies: None,
                obj_data: vec![],
            })),
            change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
        };
        let mut change_1_bytes = Vec::new();
        change_1.encode(&mut change_1_bytes).unwrap();
        let change_2 = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                id: id_2.clone(),
                dependencies: None,
                obj_data: String::from("modified").into_bytes(),
            })),
            change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
        };
        let mut change_2_bytes = Vec::new();
        change_2.encode(&mut change_2_bytes).unwrap();
        let change_3 = ChangeMsg {
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                id: id_3.clone(),
                dependencies: None,
                obj_data: String::from("modified").into_bytes(),
            })),
            change_source: Some(change_msg::ChangeSource::UserAction(EmptyMsg {})),
        };
        let mut change_3_bytes = Vec::new();
        change_3.encode(&mut change_3_bytes).unwrap();
        update_object_cache(&mut conn, &file, 1, &change_1_bytes)
            .await
            .unwrap();
        update_object_cache(&mut conn, &file, 2, &change_2_bytes)
            .await
            .unwrap();
        update_object_cache(&mut conn, &file, 3, &change_3_bytes)
            .await
            .unwrap();

        let mut answer_set = std::collections::HashSet::new();
        answer_set.insert(id_1.clone());
        answer_set.insert(id_2.clone());
        answer_set.insert(id_3.clone());
        let stream = get_latest_obj_list(conn.clone(), file.clone());
        futures::pin_mut!(stream);
        while let Some(msg_res) = stream.next().await {
            let msg_id = msg_res.unwrap();
            assert!(answer_set.remove(&msg_id));
        }
        assert_eq!(answer_set.len(), 0);
    }
}
