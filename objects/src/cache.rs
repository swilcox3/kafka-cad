//! The object cache for a file maps objIDs to a list of the last X number of changes to that object.
//! X is configurable.  

use log::*;
use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use thiserror::Error;

pub mod objects {
    tonic::include_proto!("objects");
}

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

async fn store_object_change(
    conn: &mut MultiplexedConnection,
    file: &str,
    change: u64,
    key: &str,
    obj: &[u8],
    max_len: u64,
) -> Result<(), ObjError> {
    let obj_cache = obj_cache(file, key);
    let size: u64 = conn.rpush(&obj_cache, (change, obj)).await?;
    if size > max_len {
        conn.lpop(&obj_cache).await?;
    }
    Ok(())
}

async fn get_object(
    conn: &mut MultiplexedConnection,
    file: &str,
    change: u64,
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
        let (cache_change, obj): (u64, Vec<u8>) =
            conn.lrange(&obj_cache, next_to_cur, cur_index).await?;
        if cache_change <= change {
            return Ok(obj);
        }
    }
    Err(ObjError::ObjNotFound(String::from(key)))
}

pub async fn update_object_cache(
    conn: &mut MultiplexedConnection,
    file: &str,
    change: u64,
    input: &[u8],
    max_len: u64,
) -> Result<(), ObjError> {
    let object = objects::ChangeMsg::decode(input)?;
    info!("Object received: {:?}", object);
    store_object_change(conn, file, change, &object.id, input, max_len).await?;
    Ok(())
}

pub async fn get_objects(
    conn: &mut MultiplexedConnection,
    input: &GetObjectsInput,
) -> Result<Vec<OptionChangeMsg>, ObjError> {
    let mut results = Vec::new();
    for key in &input.obj_ids {
        let mut current = OptionChangeMsg { change: None };
        match get_object(conn, &input.file, input.change_id, &key).await {
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
