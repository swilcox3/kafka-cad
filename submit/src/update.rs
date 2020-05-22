use super::*;
use indexmap::IndexMap;
use std::collections::HashSet;

fn extract_info(changes: Vec<ChangeMsg>) -> (Vec<RefIdMsg>, IndexMap<String, ChangeMsg>) {
    let mut ref_ids = Vec::new();
    let mut objects = IndexMap::new();
    for change in changes {
        if let Some(change_type) = &change.change_type {
            match change_type {
                change_msg::ChangeType::Add(object) | change_msg::ChangeType::Modify(object) => {
                    if let Some(deps) = &object.dependencies {
                        for opt_ref in &deps.references {
                            if let Some(refer) = &opt_ref.reference {
                                if let Some(owner) = &refer.owner {
                                    ref_ids.push(owner.clone())
                                }
                            }
                        }
                    }
                    objects.insert(object.id.clone(), change);
                }
                change_msg::ChangeType::Delete(msg) => {
                    objects.insert(msg.id.clone(), change);
                }
            }
        }
    }
    (ref_ids, objects)
}

async fn get_all_dependencies(
    dep_client: &mut DepClient,
    span: &Span,
    file: &String,
    offset: i64,
    ids: Vec<RefIdMsg>,
) -> Result<Vec<ReferenceMsg>, tonic::Status> {
    let input = GetAllDependenciesInput {
        file: file.clone(),
        offset,
        ids,
    };
    let resp = dep_client
        .get_all_dependencies(TracedRequest::new(input, span))
        .await;
    let refers = trace_response(resp)?.references;
    Ok(refers)
}

fn get_obj_ids_to_fetch(
    refers: &Vec<ReferenceMsg>,
    objects: &IndexMap<String, ChangeMsg>,
) -> Vec<String> {
    let mut results = HashSet::new();
    for refer in refers {
        if let Some(owner) = &refer.owner {
            if !objects.contains_key(&owner.id) {
                results.insert(owner.id.clone());
            }
        }
        if let Some(other) = &refer.other {
            if !objects.contains_key(&other.id) {
                results.insert(other.id.clone());
            }
        }
    }
    results.into_iter().collect()
}

async fn get_objects_to_update(
    obj_client: &mut ObjClient,
    span: &Span,
    file: &String,
    offset: i64,
    user: String,
    obj_ids: Vec<String>,
) -> Result<Vec<ChangeMsg>, tonic::Status> {
    let mut entries = Vec::new();
    for id in obj_ids {
        entries.push(ObjectAtOffset { offset, obj_id: id });
    }
    let input = GetObjectsInput {
        file: file.clone(),
        obj_ids: entries,
    };
    let resp = obj_client
        .get_objects(TracedRequest::new(input, span))
        .await;
    let objs_msg = trace_response(resp)?;
    let mut results = Vec::new();
    for change_opt in objs_msg.objects {
        if let Some(mut change) = change_opt.change {
            change.user = user.clone();
            results.push(change);
        }
    }
    Ok(results)
}

async fn update(
    ops_client: &mut OpsClient,
    span: &Span,
    obj_refs: Vec<ReferenceMsg>,
    objects: Vec<ChangeMsg>,
) -> Result<Vec<ChangeMsg>, tonic::Status> {
    let input = UpdateObjectsInput { obj_refs, objects };
    let resp = ops_client
        .update_objects(TracedRequest::new(input, span))
        .await;
    let output = trace_response(resp)?;
    Ok(output.objects)
}

pub async fn update_changes(
    obj_client: &mut ObjClient,
    dep_client: &mut DepClient,
    ops_client: &mut OpsClient,
    span: &Span,
    file: String,
    user: String,
    offset: i64,
    changes: Vec<ChangeMsg>,
) -> Result<Vec<ChangeMsg>, tonic::Status> {
    let (ref_ids, mut objects) = extract_info(changes);
    trace!("Got ref ids: {:?}", ref_ids);
    let refers = get_all_dependencies(dep_client, span, &file, offset, ref_ids).await?;
    trace!("Got references: {:?}", refers);
    let obj_ids = get_obj_ids_to_fetch(&refers, &objects);
    trace!("Fetching objects: {:?}", obj_ids);
    let mut fetched_objs = get_objects_to_update(obj_client, span, &file, offset, user, obj_ids).await?;
    let mut obj_vec: Vec<ChangeMsg> = objects.drain(..).map(|(_, val)| val).collect();
    obj_vec.append(&mut fetched_objs);
    debug!("Updating objects: {:?}", obj_vec);
    let results = update(ops_client, span, refers, obj_vec).await?;
    Ok(results)
}
