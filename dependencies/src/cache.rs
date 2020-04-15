use indexmap::IndexSet;
use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use std::collections::{HashSet, VecDeque};
use thiserror::Error;

pub mod object_state {
    tonic::include_proto!("object_state");
}

pub use object_state::*;

#[derive(Debug, Error)]
pub enum DepError {
    #[error("Prost encode error: {0}")]
    ProstEncodeError(#[from] prost::EncodeError),
    #[error("Prost decode error: {0}")]
    ProstDecodeError(#[from] prost::DecodeError),
    #[error("Redis error: {0:?}")]
    DatabaseError(#[from] redis::RedisError),
    #[error("Bincode error: {0}")]
    BincodeError(#[from] bincode::Error),
    #[error("RefID {0:?} not found for offset {1}")]
    DepNotFound(RefIdMsg, i64),
    #[error("Refs not found for obj {0} in file {1}")]
    RefsNotFound(String, String),
}

fn ref_id_subscribers(file: &str, ref_id: &RefIdMsg) -> String {
    format!("{}:{:?}:subs", file, ref_id)
}

fn obj_refs(file: &str, obj: &str) -> String {
    format!("{}:{}:deps", file, obj)
}

async fn update_ref_id_subscribers(
    conn: &mut MultiplexedConnection,
    file: &str,
    ref_id: &RefIdMsg,
    offset: i64,
    subs: &HashSet<Vec<u8>>,
    max_len: u64,
) -> Result<(), DepError> {
    let ref_id_subscribers = ref_id_subscribers(file, ref_id);
    let serialized_subs = bincode::serialize(subs)?;
    let size: u64 = conn
        .rpush(&ref_id_subscribers, (offset, serialized_subs))
        .await?;
    if size > max_len {
        conn.lpop(&ref_id_subscribers).await?;
    }
    Ok(())
}

async fn get_ref_id_subs(
    conn: &mut MultiplexedConnection,
    file: &str,
    ref_id: &RefIdMsg,
    before_or_equal: i64,
) -> Result<Vec<Vec<u8>>, DepError> {
    let ref_id_subscribers = ref_id_subscribers(file, ref_id);
    let cache_length: u64 = conn.llen(&ref_id_subscribers).await?;
    if cache_length == 0 {
        return Err(DepError::DepNotFound(ref_id.clone(), before_or_equal));
    }
    for i in 0isize..cache_length as isize {
        let cur_index = -1 - i;
        let next_to_cur = cur_index - 1;
        let (cache_offset, subs): (i64, Vec<u8>) = conn
            .lrange(&ref_id_subscribers, next_to_cur, cur_index)
            .await?;
        if cache_offset <= before_or_equal {
            return Ok(bincode::deserialize(&subs)?);
        }
    }
    Err(DepError::DepNotFound(ref_id.clone(), before_or_equal))
}

async fn store_obj_refs(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj: &str,
    refs: DependenciesMsg,
    offset: i64,
) -> Result<(), DepError> {
    let obj_refs = obj_refs(file, obj);
    let mut serialized = Vec::new();
    refs.encode(&mut serialized)?;
    conn.set(&obj_refs, (offset, serialized)).await?;
    Ok(())
}

async fn get_obj_refs(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj: &str,
) -> Result<(i64, DependenciesMsg), DepError> {
    let obj_refs = obj_refs(file, obj);
    let refs: Option<(i64, Vec<u8>)> = conn.get(&obj_refs).await?;
    match refs {
        Some((offset, ref_bytes)) => {
            let deserialized = DependenciesMsg::decode(ref_bytes.as_ref())?;
            Ok((offset, deserialized))
        }
        None => Err(DepError::RefsNotFound(obj.to_string(), file.to_string())),
    }
}
