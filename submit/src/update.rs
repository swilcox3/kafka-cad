use super::*;
use std::collections::{HashMap, HashSet};

fn extract_info(changes: Vec<ChangeMsg>) -> (Vec<RefIdMsg>, HashMap<String, ChangeMsg>) {
    let mut ref_ids = Vec::new();
    let mut objects = HashMap::new();
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
                }
                change_msg::ChangeType::Delete(..) => (),
            }
        }
        objects.insert(change.id.clone(), change);
    }
    (ref_ids, objects)
}

async fn get_all_dependencies(
    dep_client: &mut DepClient,
    file: &String,
    offset: i64,
    ids: Vec<RefIdMsg>,
) -> Result<Vec<ReferenceMsg>, tonic::Status> {
    let input = GetAllDependenciesInput {
        file: file.clone(),
        offset,
        ids,
    };
    let refers = dep_client
        .get_all_dependencies(Request::new(input))
        .await?
        .into_inner()
        .references;
    Ok(refers)
}

fn get_obj_ids_to_fetch(
    refers: &Vec<ReferenceMsg>,
    objects: &HashMap<String, ChangeMsg>,
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
    let objs_msg = obj_client
        .get_objects(Request::new(input))
        .await?
        .into_inner();
    let mut results = Vec::new();
    for change_opt in objs_msg.objects {
        if let Some(mut change) = change_opt.change {
            change.user = user.clone();
            results.push(change);
        }
    }
    Ok(results)
}

fn get_object(change: &ChangeMsg) -> Option<&ObjectMsg> {
    let mut result = None;
    if let Some(change_type) = &change.change_type {
        match change_type {
            change_msg::ChangeType::Add(object) | change_msg::ChangeType::Modify(object) => {
                result = Some(object);
            }
            _ => (),
        }
    }
    result
}

fn get_object_mut(change: &mut ChangeMsg) -> Option<&mut ObjectMsg> {
    let mut result = None;
    if let Some(change_type) = &mut change.change_type {
        match change_type {
            change_msg::ChangeType::Add(object) | change_msg::ChangeType::Modify(object) => {
                result = Some(object);
            }
            _ => (),
        }
    }
    result
}

async fn update(
    ops_client: &mut OpsClient,
    obj_refs: Vec<ReferenceMsg>,
    objects: Vec<ChangeMsg>,
) -> Result<Vec<ChangeMsg>, tonic::Status> {
    let input = UpdateObjectsInput { obj_refs, objects };
    let output = ops_client
        .update_objects(Request::new(input))
        .await?
        .into_inner();
    Ok(output.objects)
}

pub async fn update_changes(
    obj_client: &mut ObjClient,
    dep_client: &mut DepClient,
    ops_client: &mut OpsClient,
    file: String,
    user: String,
    offset: i64,
    changes: Vec<ChangeMsg>,
) -> Result<Vec<ChangeMsg>, tonic::Status> {
    let (ref_ids, mut objects) = extract_info(changes);
    let refers = get_all_dependencies(dep_client, &file, offset, ref_ids).await?;
    let obj_ids = get_obj_ids_to_fetch(&refers, &objects);
    let obj_vec = get_objects_to_update(obj_client, &file, offset, user, obj_ids).await?;
    let results = update(ops_client, refers, obj_vec).await?;
    Ok(results)
}
