use indexmap::IndexSet;
use log::*;
use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use thiserror::Error;

pub mod geom {
    tonic::include_proto!("geom");
}

pub mod object_state {
    tonic::include_proto!("object_state");

    impl From<super::RefID> for RefIdMsg {
        fn from(id: super::RefID) -> RefIdMsg {
            RefIdMsg {
                id: id.id,
                ref_type: id.ref_type,
                index: id.index,
            }
        }
    }

    impl From<&super::RefID> for RefIdMsg {
        fn from(id: &super::RefID) -> RefIdMsg {
            RefIdMsg {
                id: id.id.clone(),
                ref_type: id.ref_type,
                index: id.index,
            }
        }
    }

    impl From<super::Reference> for ReferenceMsg {
        fn from(refer: super::Reference) -> ReferenceMsg {
            ReferenceMsg {
                owner: Some(RefIdMsg::from(refer.owner)),
                other: Some(RefIdMsg::from(refer.other)),
            }
        }
    }
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
}

impl Into<tonic::Status> for DepError {
    fn into(self) -> tonic::Status {
        let msg = format!("{}", self);
        let code = match self {
            DepError::DatabaseError(..)
            | DepError::BincodeError(..)
            | DepError::ProstEncodeError(..)
            | DepError::ProstDecodeError(..) => tonic::Code::Internal,
        };
        tonic::Status::new(code, msg)
    }
}

pub fn to_status<T: Into<DepError>>(err: T) -> tonic::Status {
    let obj_error: DepError = err.into();
    obj_error.into()
}

///Anti-corruption layer.  Also, RefIdMsg doesn't implement Hash by default.
#[derive(Debug, Clone, Hash, Serialize, Deserialize, PartialEq, Eq)]
struct RefID {
    id: String,
    ref_type: i32,
    index: u64,
}

impl From<RefIdMsg> for RefID {
    fn from(msg: RefIdMsg) -> RefID {
        RefID {
            id: msg.id,
            ref_type: msg.ref_type,
            index: msg.index,
        }
    }
}

impl From<&RefIdMsg> for RefID {
    fn from(msg: &RefIdMsg) -> RefID {
        RefID {
            id: msg.id.clone(),
            ref_type: msg.ref_type,
            index: msg.index,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Reference {
    owner: RefID,
    other: RefID,
}

impl std::hash::Hash for Reference {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.owner.hash(state);
        self.other.hash(state);
    }
}

impl std::cmp::Eq for Reference {}

fn ref_id_subscribers(file: &str, ref_id: &RefID) -> String {
    format!("{}:{:?}:subs", file, ref_id)
}

fn obj_refs(file: &str, obj: &str) -> String {
    format!("{}:{}:deps", file, obj)
}

#[derive(Debug, Serialize, Deserialize)]
struct Subscribers {
    offset: i64,
    subs: HashSet<RefID>,
}

async fn update_ref_id_subscribers(
    conn: &mut MultiplexedConnection,
    file: &str,
    ref_id: &RefID,
    offset: i64,
    subs: HashSet<RefID>,
) -> Result<(), DepError> {
    debug!(
        "Updating subs {:?} for ref ID {:?} in file {}",
        subs, ref_id, file
    );
    let ref_id_subscribers = ref_id_subscribers(file, ref_id);
    let entry = Subscribers { offset, subs };
    let serialized_subs = bincode::serialize(&entry)?;
    //Push to the left so the latest is first in the list
    conn.lpush(&ref_id_subscribers, serialized_subs).await?;
    Ok(())
}

async fn get_ref_id_subs(
    conn: &mut MultiplexedConnection,
    file: &str,
    ref_id: &RefID,
    before_or_equal: i64,
) -> Result<HashSet<RefID>, DepError> {
    debug!("Getting subs for ref ID {:?} in file {}", ref_id, file);
    let ref_id_subscribers = ref_id_subscribers(file, ref_id);
    let cache_length: u64 = conn.llen(&ref_id_subscribers).await?;
    for i in 0isize..cache_length as isize {
        let raw_bytes: Vec<u8> = conn.lindex(&ref_id_subscribers, i).await?;
        let entry: Subscribers = bincode::deserialize(&raw_bytes)?;
        if entry.offset <= before_or_equal {
            return Ok(entry.subs);
        }
    }
    Ok(HashSet::new())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ObjRefs {
    offset: i64,
    serialized_refs: Vec<u8>,
}

async fn store_obj_refs(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj_id: &str,
    refs: &DependenciesMsg,
    offset: i64,
) -> Result<(), DepError> {
    debug!("Storing refs for object {} from file {}", obj_id, file);
    let obj_refs = obj_refs(file, obj_id);
    let mut serialized_refs = Vec::new();
    refs.encode(&mut serialized_refs)?;
    let refs = ObjRefs {
        offset,
        serialized_refs,
    };
    conn.set(&obj_refs, bincode::serialize(&refs)?).await?;
    Ok(())
}

async fn get_obj_refs(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj_id: &str,
) -> Result<Option<(i64, DependenciesMsg)>, DepError> {
    debug!("Getting refs for object {} from file {}", obj_id, file);
    let obj_refs = obj_refs(file, obj_id);
    let refs: Option<Vec<u8>> = conn.get(&obj_refs).await?;
    match refs {
        Some(raw_bytes) => {
            let obj_refs: ObjRefs = bincode::deserialize(&raw_bytes)?;
            let deserialized = DependenciesMsg::decode(obj_refs.serialized_refs.as_ref())?;
            Ok(Some((obj_refs.offset, deserialized)))
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
    changed_subs: &mut HashMap<RefID, HashSet<RefID>>,
) -> Result<(), DepError> {
    if let Some(ref_owner) = &refer.owner {
        if let Some(ref_other) = &refer.other {
            let ref_owner = RefID::from(ref_owner);
            let ref_other = RefID::from(ref_other);
            if !changed_subs.contains_key(&ref_other) {
                match get_ref_id_subs(conn, file, &ref_other, offset).await {
                    Ok(subs) => {
                        debug!(
                            "Got subs {:#?} for ref_id {:?} from file {}",
                            subs, ref_other, file
                        );
                        changed_subs.insert(ref_other.clone(), subs);
                    }
                    Err(e) => return Err(e),
                }
            }
            if let Some(subs) = changed_subs.get_mut(&ref_other) {
                match change_type {
                    DepChange::Add | DepChange::Modify => {
                        subs.insert(ref_owner);
                    }
                    DepChange::Delete => {
                        subs.remove(&ref_owner);
                    }
                }
            }
        }
    }
    Ok(())
}

async fn add_deps(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj_id: &str,
    deps: &DependenciesMsg,
    offset: i64,
) -> Result<(), DepError> {
    debug!(
        "Adding dependencies {:#?} for object {} from file {}",
        deps, obj_id, file
    );
    let mut changed_subs = HashMap::new();
    for refer_opt in &deps.references {
        if let Some(refer) = &refer_opt.reference {
            populate_changed_subs(conn, file, offset, refer, DepChange::Add, &mut changed_subs)
                .await?;
        }
    }
    store_obj_refs(conn, file, obj_id, deps, offset).await?;
    for (ref_id, subs) in changed_subs {
        update_ref_id_subscribers(conn, file, &ref_id, offset, subs).await?;
    }
    Ok(())
}

async fn modify_deps(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj_id: &str,
    deps: &DependenciesMsg,
    offset: i64,
) -> Result<(), DepError> {
    debug!(
        "Modifying dependencies {:?} for object {} from file {}",
        deps, obj_id, file
    );
    let mut changed_subs = HashMap::new();
    match get_obj_refs(conn, file, obj_id).await? {
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
    store_obj_refs(conn, file, &obj_id, &deps, offset).await?;
    for (ref_id, subs) in changed_subs {
        update_ref_id_subscribers(conn, file, &ref_id, offset, subs).await?;
    }
    Ok(())
}

async fn delete_deps(
    conn: &mut MultiplexedConnection,
    file: &str,
    obj_id: &str,
    deps: &DependenciesMsg,
    offset: i64,
) -> Result<(), DepError> {
    debug!(
        "Deleting dependencies {:?} for object {} from file {}",
        deps, obj_id, file
    );
    let mut changed_subs = HashMap::new();
    for refer_opt in &deps.references {
        if let Some(refer) = &refer_opt.reference {
            populate_changed_subs(
                conn,
                file,
                offset,
                refer,
                DepChange::Delete,
                &mut changed_subs,
            )
            .await?;
        }
    }
    let mut delete_deps = DependenciesMsg {
        references: Vec::with_capacity(deps.references.len()),
    };
    delete_deps
        .references
        .resize_with(deps.references.len(), || OptionReferenceMsg {
            reference: None,
        });
    store_obj_refs(conn, file, obj_id, &delete_deps, offset).await?;
    for (ref_id, subs) in changed_subs {
        update_ref_id_subscribers(conn, file, &ref_id, offset, subs).await?;
    }

    Ok(())
}

async fn update_deps_inner(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    change: ChangeMsg,
) -> Result<(), DepError> {
    if let Some(change_type) = change.change_type {
        match change_type {
            change_msg::ChangeType::Add(object) => {
                if let Some(deps) = object.dependencies {
                    add_deps(conn, file, &change.id, &deps, offset).await?;
                }
            }
            change_msg::ChangeType::Modify(object) => {
                if let Some(deps) = object.dependencies {
                    modify_deps(conn, file, &change.id, &deps, offset).await?;
                }
            }
            change_msg::ChangeType::Delete(..) => {
                let prev_obj_refs = get_obj_refs(conn, file, &change.id).await?;
                if let Some((_, deps)) = prev_obj_refs {
                    delete_deps(conn, file, &change.id, &deps, offset).await?;
                }
            }
        }
    }
    Ok(())
}

pub async fn update_deps(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    change: &[u8],
) -> Result<(), DepError> {
    let change_msg = ChangeMsg::decode(change)?;
    update_deps_inner(conn, file, offset, change_msg).await?;
    Ok(())
}

async fn breadth_first_search(
    conn: &mut MultiplexedConnection,
    file: &str,
    offset: i64,
    ref_id: RefID,
) -> Result<IndexSet<Reference>, DepError> {
    let mut processing = VecDeque::new();
    let mut visited = HashSet::new();
    let mut result = IndexSet::new();
    visited.insert(ref_id.clone());
    processing.push_back(ref_id);
    while processing.len() > 0 {
        if let Some(current) = processing.pop_front() {
            let sub_set = get_ref_id_subs(conn, file, &current, offset).await?;
            for sub in sub_set {
                if let None = visited.get(&sub) {
                    visited.insert(sub.clone());
                    let refer = Reference {
                        owner: sub.clone(),
                        other: current.clone(),
                    };
                    result.insert(refer);
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
    offset: i64,
    ref_ids: &Vec<RefIdMsg>,
) -> Result<Vec<ReferenceMsg>, DepError> {
    let mut result_set = IndexSet::new();
    for ref_id in ref_ids {
        let set = breadth_first_search(conn, file, offset, RefID::from(ref_id)).await?;
        result_set.extend(set);
    }
    let mut results = Vec::new();
    for refer in result_set {
        let refer_msg = ReferenceMsg::from(refer);
        results.push(refer_msg);
    }
    Ok(results)
}
