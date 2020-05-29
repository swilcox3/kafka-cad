//! The object cache for a file maps objIDs to a list of the last X number of changes to that object.
//! X is configurable.

use super::*;
use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepCacheError {
    #[error("Object {0} not found")]
    ObjNotFound(String),
    #[error("Prost encode error: {0}")]
    ProstEncodeError(#[from] prost::EncodeError),
    #[error("Prost decode error: {0}")]
    ProstDecodeError(#[from] prost::DecodeError),
    #[error("Redis error: {0:?}")]
    DatabaseError(#[from] redis::RedisError),
}

impl Into<tonic::Status> for RepCacheError {
    fn into(self) -> tonic::Status {
        let msg = format!("{}", self);
        let code = match self {
            RepCacheError::DatabaseError(..)
            | RepCacheError::ProstEncodeError(..)
            | RepCacheError::ProstDecodeError(..) => tonic::Code::Internal,
            RepCacheError::ObjNotFound(..) => tonic::Code::NotFound,
        };
        tonic::Status::new(code, msg)
    }
}

pub fn to_status<T: Into<RepCacheError>>(err: T) -> tonic::Status {
    let obj_error: RepCacheError = err.into();
    obj_error.into()
}

fn obj_rep_cache(file: &str, key: &str) -> String {
    format!("{}:{}", file, key)
}

async fn store_object_rep(
    conn: &mut MultiplexedConnection,
    file: &str,
    key: &str,
    obj: &[u8],
) -> Result<(), RepCacheError> {
    let obj_rep_cache = obj_rep_cache(file, key);
    trace!("Setting obj {} rep in file{}", key, file);
    conn.set(&obj_rep_cache, obj).await?;
    Ok(())
}

pub async fn get_object_rep(
    conn: &mut MultiplexedConnection,
    file: &str,
    key: &str,
) -> Result<representation::UpdateChangeMsg, RepCacheError> {
    trace!("getting object {} in file {}", key, file,);
    let obj_rep_cache = obj_rep_cache(file, key);
    let rep_opt: Option<Vec<u8>> = conn.get(obj_rep_cache).await?;
    match rep_opt {
        Some(rep_bin) => {
            let rep = representation::UpdateChangeMsg::decode(rep_bin.as_ref())?;
            Ok(rep)
        }
        None => Err(RepCacheError::ObjNotFound(String::from(key))),
    }
}

pub async fn update_object_rep_cache(
    conn: &mut MultiplexedConnection,
    file: &str,
    input: &[u8],
) -> Result<(), RepCacheError> {
    let object = representation::UpdateChangeMsg::decode(input)?;
    info!("Updating object rep cache: {:?}", object);
    store_object_rep(conn, file, &object.obj_id, input).await?;
    Ok(())
}
