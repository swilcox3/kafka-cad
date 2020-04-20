use super::*;
use std::collections::{HashMap, HashSet};

fn get_all_ref_ids(changes: &Vec<ChangeMsg>) -> Vec<RefIdMsg> {
    let mut results = Vec::new();
    for change in changes {
        if let Some(change_type) = &change.change_type {
            match change_type {
                change_msg::ChangeType::Add(object)
                | change_msg::ChangeType::Modify(object)
                | change_msg::ChangeType::Delete(object) => {
                    if let Some(deps) = &object.dependencies {
                        for opt_ref in &deps.references {
                            if let Some(refer) = &opt_ref.reference {
                                if let Some(owner) = &refer.owner {
                                    results.push(owner.clone())
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    results
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

fn get_all_obj_ids(refers: &Vec<ReferenceMsg>) -> Vec<String> {
    let mut results = HashSet::new();
    for refer in refers {
        if let Some(owner) = &refer.owner {
            results.insert(owner.id.clone());
        }
        if let Some(other) = &refer.other {
            results.insert(other.id.clone());
        }
    }
    results.into_iter().collect()
}

async fn get_all_objects(
    obj_client: &mut ObjClient,
    file: &String,
    offset: i64,
    obj_ids: Vec<String>,
) -> Result<HashMap<String, ObjectMsg>, tonic::Status> {
    let input = GetObjectsInput {
        file: file.clone(),
        offset,
        obj_ids,
    };
    let objs_msg = obj_client
        .get_objects(Request::new(input))
        .await?
        .into_inner();
    let mut results = HashMap::new();
    for change_opt in objs_msg.objects {
        if let Some(change) = change_opt.change {
            if let Some(change_type) = change.change_type {
                match change_type {
                    change_msg::ChangeType::Add(object)
                    | change_msg::ChangeType::Modify(object) => {
                        results.insert(change.id, object);
                    }
                    _ => (),
                }
            }
        }
    }
    Ok(results)
}

fn update(refers: &Vec<ReferenceMsg>, objects: &mut HashMap<String, ObjectMsg>) {
    for refer in refers {
        if let Some(other) = refer.other {
            if let Some(owner) = refer.owner {
                if let Some(update_type) = refer.update_type {
                    let other_obj_opt = objects.get(&other.id);
                    if let Some(owner_obj) = objects.get(&owner.id) {
                        match other_obj_opt {
                            Some(other_obj) => {}
                            None => {
                                if let Some(deps) = owner_obj.dependencies {
                                    for refer_opt in deps.references {
                                        let mut delete = false;
                                        if let Some(existing_ref) = refer_opt.reference {
                                            if existing_ref == *refer {
                                                delete = true;
                                            }
                                        }
                                        if delete {
                                            refer_opt.reference = None;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub async fn update_changes(
    obj_client: &mut ObjClient,
    dep_client: &mut DepClient,
    file: String,
    user: String,
    offset: i64,
    changes: Vec<ChangeMsg>,
) -> Result<Vec<ChangeMsg>, tonic::Status> {
    let ref_ids = get_all_ref_ids(&changes);
    let refers = get_all_dependencies(dep_client, &file, offset, ref_ids).await?;
    let obj_ids = get_all_obj_ids(&refers);
    let objects = get_all_objects(obj_client, &file, offset, obj_ids).await?;
    Ok(Vec::new())
}
