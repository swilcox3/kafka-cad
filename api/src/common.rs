use super::*;

use object_state::*;
use objects::*;
use submit::*;
use tonic::transport::Channel;
use tonic::Status;

pub async fn undo_client(
    undo_url: String,
) -> Result<undo::undo_client::UndoClient<Channel>, Status> {
    let undo_client = undo::undo_client::UndoClient::connect(undo_url)
        .instrument(info_span!("undo_client::connect"))
        .await
        .map_err(unavailable)?;
    Ok(undo_client)
}

pub async fn submit_client(
    submit_url: String,
) -> Result<submit::submit_changes_client::SubmitChangesClient<Channel>, Status> {
    let submit_client = submit::submit_changes_client::SubmitChangesClient::connect(submit_url)
        .instrument(info_span!("submit::connect"))
        .await
        .map_err(unavailable)?;
    Ok(submit_client)
}

pub async fn rep_cache_client(
    rep_cache_url: String,
) -> Result<rep_cache::rep_cache_client::RepCacheClient<Channel>, Status> {
    let rep_cache_client = rep_cache::rep_cache_client::RepCacheClient::connect(rep_cache_url)
        .instrument(info_span!("rep_cache_client::connect"))
        .await
        .map_err(unavailable)?;
    Ok(rep_cache_client)
}

pub async fn objects_client(
    objects_url: String,
) -> Result<objects::objects_client::ObjectsClient<Channel>, Status> {
    let obj_client = objects::objects_client::ObjectsClient::connect(objects_url)
        .instrument(info_span!("obj_client::connect"))
        .await
        .map_err(unavailable)?;
    Ok(obj_client)
}

pub async fn operations_client(
    operations_url: String,
) -> Result<operations::operations_client::OperationsClient<Channel>, Status> {
    let ops_client = operations::operations_client::OperationsClient::connect(operations_url)
        .instrument(info_span!("ops_client::connect"))
        .await
        .map_err(unavailable)?;
    Ok(ops_client)
}

pub async fn get_objects(
    client: &mut objects_client::ObjectsClient<Channel>,
    file: &str,
    obj_ids: Vec<String>,
    offset: i64,
    error_on_missing: bool,
) -> Result<Vec<ObjectMsg>, Status> {
    let mut obj_offsets = Vec::new();
    for obj in &obj_ids {
        obj_offsets.push(objects::ObjectAtOffset {
            offset,
            obj_id: obj.clone(),
        });
    }
    let resp = client
        .get_objects(TracedRequest::new(objects::GetObjectsInput {
            file: String::from(file),
            obj_ids: obj_offsets,
        }))
        .await;
    let changes = trace_response(resp)?;
    let mut objects = Vec::new();
    for (change_opt, obj_id) in changes.objects.into_iter().zip(obj_ids.into_iter()) {
        match change_opt.change {
            Some(change) => match change.change_type {
                Some(change_msg::ChangeType::Add(msg))
                | Some(change_msg::ChangeType::Modify(msg)) => objects.push(msg),
                Some(change_msg::ChangeType::Delete(msg)) => {
                    if error_on_missing {
                        return Err(Status::not_found(format!(
                            "Object {:?} has been deleted",
                            msg.id
                        )));
                    } else {
                        warn!("Object {:?} has been deleted, skipping", msg.id);
                    }
                }
                None => {
                    if error_on_missing {
                        return Err(Status::not_found(format!(
                            "Object {:?} has no data set",
                            obj_id
                        )));
                    } else {
                        warn!("Object {:?} has no data set, skipping", obj_id);
                    }
                }
            },
            None => {
                if error_on_missing {
                    return Err(Status::not_found(format!("Object {:?} not found", obj_id)));
                } else {
                    warn!("Object {:?} not found, skipping", obj_id);
                }
            }
        }
    }
    Ok(objects)
}

pub async fn submit_changes(
    client: &mut submit_changes_client::SubmitChangesClient<Channel>,
    file: String,
    user: String,
    offset: i64,
    changes: Vec<ChangeMsg>,
) -> Result<i64, Status> {
    if changes.len() == 0 {
        return Err(Status::aborted(format!("No changes to submit")));
    }
    let resp = client
        .submit_changes(TracedRequest::new(submit::SubmitChangesInput {
            file,
            user,
            offset,
            changes,
        }))
        .await;
    let mut output = trace_response(resp)?;
    match output.offsets.pop() {
        Some(offset) => Ok(offset),
        None => Err(Status::out_of_range(
            "No offsets received from submit service",
        )),
    }
}

pub fn add(user: &str, obj: ObjectMsg) -> ChangeMsg {
    object_state::ChangeMsg {
        user: String::from(user),
        change_type: Some(object_state::change_msg::ChangeType::Add(obj)),
        change_source: Some(object_state::change_msg::ChangeSource::UserAction(
            object_state::EmptyMsg {},
        )),
    }
}

pub fn modify(user: &str, obj: ObjectMsg) -> ChangeMsg {
    object_state::ChangeMsg {
        user: String::from(user),
        change_type: Some(object_state::change_msg::ChangeType::Modify(obj)),
        change_source: Some(object_state::change_msg::ChangeSource::UserAction(
            object_state::EmptyMsg {},
        )),
    }
}

pub fn delete(user: &str, obj_id: String) -> ChangeMsg {
    object_state::ChangeMsg {
        user: String::from(user),
        change_type: Some(object_state::change_msg::ChangeType::Delete(DeleteMsg {
            id: obj_id,
        })),
        change_source: Some(object_state::change_msg::ChangeSource::UserAction(
            object_state::EmptyMsg {},
        )),
    }
}
