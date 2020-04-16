use indexmap::IndexSet;
use log::*;
use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use std::collections::{HashMap, HashSet, VecDeque};
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
    #[error("Refs not found for obj {0} in file {1}")]
    RefsNotFound(String, String),
}

fn ref_id_subscribers(file: &str, ref_id: &[u8]) -> String {
    format!("{}:{:?}:subs", file, ref_id)
}

fn obj_refs(file: &str, obj: &str) -> String {
    format!("{}:{}:deps", file, obj)
}

async fn update_ref_id_subscribers(
    conn: &mut MultiplexedConnection,
    file: &str,
    ref_id: &[u8],
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
    ref_id: &[u8],
    before_or_equal: i64,
) -> Result<HashSet<Vec<u8>>, DepError> {
    let ref_id_subscribers = ref_id_subscribers(file, ref_id);
    let cache_length: u64 = conn.llen(&ref_id_subscribers).await?;
    let mut results = HashSet::new();
    for i in 0isize..cache_length as isize {
        let cur_index = -1 - i;
        let next_to_cur = cur_index - 1;
        let (cache_offset, subs): (i64, Vec<u8>) = conn
            .lrange(&ref_id_subscribers, next_to_cur, cur_index)
            .await?;
        if cache_offset <= before_or_equal {
            results = bincode::deserialize(&subs)?;
            break;
        }
    }
    Ok(results)
}

async fn store_obj_refs(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj_id: &str,
    refs: DependenciesMsg,
    offset: i64,
) -> Result<(), DepError> {
    let obj_refs = obj_refs(file, obj_id);
    let mut serialized = Vec::new();
    refs.encode(&mut serialized)?;
    conn.set(&obj_refs, (offset, serialized)).await?;
    Ok(())
}

async fn get_obj_refs(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj_id: &str,
) -> Result<Option<(i64, DependenciesMsg)>, DepError> {
    let obj_refs = obj_refs(file, obj_id);
    let refs: Option<(i64, Vec<u8>)> = conn.get(&obj_refs).await?;
    match refs {
        Some((offset, ref_bytes)) => {
            let deserialized = DependenciesMsg::decode(ref_bytes.as_ref())?;
            Ok(Some((offset, deserialized)))
        }
        None => Ok(None),
    }
}

pub enum DepChange {
    Add,
    Modify,
    Delete,
}

fn get_ref_diffs(
    new_refs: &Vec<OptionReferenceMsg>,
    old_refs: &Vec<OptionReferenceMsg>,
) -> Vec<Option<(ReferenceMsg, DepChange)>> {
    let length = std::cmp::max(new_refs.len(), old_refs.len());
    let mut new_iter = new_refs.iter();
    let mut old_iter = old_refs.iter();
    let mut results = Vec::new();
    for _ in 0..length {
        let new_ref_opt = match new_iter.next() {
            Some(opt_msg) => &opt_msg.reference,
            None => &None,
        };
        let old_ref_opt = match old_iter.next() {
            Some(opt_msg) => &opt_msg.reference,
            None => &None,
        };
        let change_opt = match new_ref_opt {
            Some(refer) => match old_ref_opt {
                Some(..) => Some((refer.clone(), DepChange::Modify)),
                None => Some((refer.clone(), DepChange::Add)),
            },
            None => match old_ref_opt {
                Some(old_refer) => Some((old_refer.clone(), DepChange::Delete)),
                None => None,
            },
        };
        results.push(change_opt);
    }
    results
}

async fn populate_changed_subs(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    refer: &ReferenceMsg,
    change_type: DepChange,
    changed_subs: &mut HashMap<Vec<u8>, HashSet<Vec<u8>>>,
) -> Result<(), DepError> {
    if let Some(ref_owner) = &refer.owner {
        if let Some(ref_other) = &refer.other {
            let mut owner_serialized = Vec::new();
            ref_owner.encode(&mut owner_serialized)?;
            let mut other_serialized = Vec::new();
            ref_other.encode(&mut other_serialized)?;
            if !changed_subs.contains_key(&other_serialized) {
                let subs = get_ref_id_subs(conn, file, &other_serialized, offset).await?;
                changed_subs.insert(other_serialized.clone(), subs);
            }
            if let Some(subs) = changed_subs.get_mut(&other_serialized) {
                match change_type {
                    DepChange::Add | DepChange::Modify => {
                        subs.insert(owner_serialized);
                    }
                    DepChange::Delete => {
                        subs.remove(&other_serialized);
                    }
                }
            }
        }
    }
    Ok(())
}

pub async fn update_deps(
    conn: &mut MultiplexedConnection,
    file: &str,
    change: ChangeMsg,
    offset: i64,
    max_len: u64,
) -> Result<(), DepError> {
    if let Some(object) = change.object {
        if let Some(deps) = object.dependencies {
            let mut changed_subs = HashMap::new();
            match get_obj_refs(conn, file, &change.id).await? {
                Some((_, old_deps)) => {
                    let diffs = get_ref_diffs(&deps.references, &old_deps.references);
                    for change_opt in diffs {
                        if let Some((refer, change_type)) = change_opt {
                            populate_changed_subs(
                                conn,
                                file,
                                offset,
                                &refer,
                                change_type,
                                &mut changed_subs,
                            )
                            .await?;
                        }
                    }
                }
                None => {
                    for refer_opt in &deps.references {
                        if let Some(refer) = &refer_opt.reference {
                            populate_changed_subs(
                                conn,
                                file,
                                offset,
                                refer,
                                DepChange::Add,
                                &mut changed_subs,
                            )
                            .await?;
                        }
                    }
                }
            }
            store_obj_refs(conn, file, &change.id, deps, offset).await?;
            for (ref_id, subs) in changed_subs {
                update_ref_id_subscribers(conn, file, &ref_id, offset, &subs, max_len).await?;
            }
        }
    }
    Ok(())
}

async fn breadth_first_search(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    ref_id: Vec<u8>,
) -> Result<IndexSet<Vec<u8>>, DepError> {
    let mut processing = VecDeque::new();
    let mut visited = HashSet::new();
    let mut result = IndexSet::new();
    visited.insert(ref_id.clone());
    processing.push_back(ref_id);
    while processing.len() > 0 {
        if let Some(current) = processing.pop_front() {
            let current_deserialized = RefIdMsg::decode(current.as_ref())?;
            let sub_set = get_ref_id_subs(conn, file, &current, offset).await?;
            for sub in sub_set {
                if let None = visited.get(&sub) {
                    visited.insert(sub.clone());
                    let sub_deserialized = RefIdMsg::decode(sub.as_ref())?;
                    let ref_msg = ReferenceMsg {
                        owner: Some(sub_deserialized),
                        other: Some(current_deserialized.clone()),
                    };
                    let mut ref_msg_serialized = Vec::new();
                    ref_msg.encode(&mut ref_msg_serialized)?;
                    result.insert(ref_msg_serialized);
                    processing.push_back(sub);
                }
            }
        }
    }
    Ok(result)
}

pub async fn get_all_deps(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset:i64,
    ref_ids: Vec<RefIdMsg>
) -> Result<Vec<ReferenceMsg>, DepError> {
    let mut result_set = IndexSet::new();
    for ref_id in ref_ids {
        let mut ref_id_serialized = Vec::new();
        ref_id.encode(&mut ref_id_serialized)?;
        let set = breadth_first_search(conn, file, offset, ref_id_serialized).await?;
        result_set.extend(set);
    }
    let mut results = Vec::new();
    for serialized_refer in result_set {
        let refer_msg = ReferenceMsg::decode(serialized_refer.as_ref())?;
        results.push(refer_msg);
    }
    Ok(results)
}
