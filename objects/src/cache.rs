//! The object cache for a file maps objIDs to a list of the last X number of changes to that object.
//! X is configurable.  

use log::*;
use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use thiserror::Error;

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
}

impl Into<tonic::Status> for ObjError {
    fn into(self) -> tonic::Status {
        let msg = format!("{}", self);
        let code = match self {
            ObjError::DatabaseError(..)
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

async fn store_object_change(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    key: &str,
    obj: &[u8],
) -> Result<(), ObjError> {
    let obj_cache = obj_cache(file, key);
    conn.rpush(&obj_cache, (offset, obj)).await?;
    Ok(())
}

async fn store_file_offset(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
) -> Result<(), ObjError> {
    let file_offset = file_offset(file);
    conn.set(file_offset, offset).await?;
    Ok(())
}

async fn get_object(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    key: &str,
) -> Result<Vec<u8>, ObjError> {
    let obj_cache = obj_cache(file, key);
    let cache_length: u64 = conn.llen(&obj_cache).await?;
    if cache_length == 0 {
        return Err(ObjError::ObjNotFound(String::from(key)));
    }
    //Iterate over cache backwards, so latest to newest.
    for i in 0isize..cache_length as isize {
        let cur_index = -1 - i;
        let next_to_cur = cur_index - 1;
        let (cache_offset, obj): (i64, Vec<u8>) =
            conn.lrange(&obj_cache, next_to_cur, cur_index).await?;
        if cache_offset <= offset {
            return Ok(obj);
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
    info!("Object received: {:?}", object);
    let id = match object.change_type {
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
    Ok(())
}

pub async fn get_objects(
    conn: &mut MultiplexedConnection,
    input: &GetObjectsInput,
) -> Result<Vec<OptionChangeMsg>, ObjError> {
    let mut results = Vec::new();
    for entry in &input.obj_ids {
        let mut current = OptionChangeMsg { change: None };
        match get_object(conn, &input.file, entry.offset, &entry.obj_id).await {
            Ok(bytes) => {
                current.change = Some(ChangeMsg::decode(bytes.as_ref())?);
            }
            Err(e) => {
                error!("{}", e);
                match e {
                    ObjError::ObjNotFound(..) => (),
                    _ => return Err(e),
                }
            }
        }
        results.push(current);
    }
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
            id: id.clone(),
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Add(ObjectMsg {
                obj_url: String::from("test"),
                dependencies: None,
                results: None,
                obj_data: vec![],
            })),
        };
        let mut change_1_bytes = Vec::new();
        change_1.encode(&mut change_1_bytes).unwrap();
        let offset_1 = 4;
        update_object_cache(&mut conn, &file, offset_1, &change_1_bytes)
            .await
            .unwrap();
        assert_eq!(
            get_object(&mut conn, &file, offset_1, &id).await.unwrap(),
            change_1_bytes
        );

        let change_2 = ChangeMsg {
            id: id.clone(),
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Modify(ObjectMsg {
                obj_url: String::from("test"),
                dependencies: None,
                results: None,
                obj_data: String::from("modified").into_bytes(),
            })),
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
            id: id.clone(),
            user: user.clone(),
            change_type: Some(change_msg::ChangeType::Delete(DeleteMsg {})),
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

        assert!(get_object(&mut conn, &file, offset_1, &id).await.is_err());
    }
}
