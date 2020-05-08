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
    objects: &mut HashMap<String, ChangeMsg>,
) -> Result<(), tonic::Status> {
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
    for change_opt in objs_msg.objects {
        if let Some(mut change) = change_opt.change {
            change.user = user.clone();
            objects.insert(change.id.clone(), change);
        }
    }
    Ok(())
}

fn get_profile_pt_to_update<'a>(
    owner: &RefIdMsg,
    results: &'a mut ResultsMsg,
) -> Option<&'a mut Point3Msg> {
    let mut result = None;
    if let Some(profile) = &mut results.profile {
        if let Some(pt_opt) = profile.points.get_mut(owner.index as usize) {
            if let Some(pt) = &mut pt_opt.pt {
                result = Some(pt);
            }
        }
    }
    result
}

fn get_pt_from_other<'a>(
    other: &RefIdMsg,
    other_results: &'a ResultsMsg,
    update_type: &reference_msg::UpdateType,
) -> Option<&'a Point3Msg> {
    let mut result = None;
    if let Some(other_ref_type) = ref_id_msg::RefType::from_i32(other.ref_type) {
        if let Some(profile) = &other_results.profile {
            match other_ref_type {
                ref_id_msg::RefType::AxisAlignedBbox => {}
                ref_id_msg::RefType::ProfilePoint => {
                    if let Some(pt_opt) = profile.points.get(other.index as usize) {
                        if let Some(pt) = &pt_opt.pt {
                            result = Some(pt);
                        }
                    }
                }
                ref_id_msg::RefType::ProfileLine => {
                    if let Some(line_opt) = profile.lines.get(other.index as usize) {
                        if let Some(line) = &line_opt.line {
                            match update_type {
                                reference_msg::UpdateType::Equals(params) => {
                                    match params.other_index {
                                        0 => {
                                            if let Some(pt) = &line.first {
                                                result = Some(pt)
                                            }
                                        }
                                        1 => {
                                            if let Some(pt) = &line.second {
                                                result = Some(pt)
                                            }
                                        }
                                        _ => (),
                                    }
                                }
                                reference_msg::UpdateType::Interp(params) => {
                                    let first_opt = match params.first_other {
                                        0 => {
                                            if let Some(pt) = &line.first {
                                                Some(pt)
                                            } else {
                                                None
                                            }
                                        }
                                        1 => {
                                            if let Some(pt) = &line.second {
                                                Some(pt)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    };
                                    let second_opt = match params.second_other {
                                        0 => {
                                            if let Some(pt) = &line.first {
                                                Some(pt)
                                            } else {
                                                None
                                            }
                                        }
                                        1 => {
                                            if let Some(pt) = &line.second {
                                                Some(pt)
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    };
                                    //TODO do interpolation math
                                    if let Some(first) = first_opt {
                                        if let Some(second) = second_opt {
                                            result = Some(first);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ref_id_msg::RefType::ProfilePlane => {}
                _ => (),
            }
        }
    }
    result
}

fn update_profile_pt(
    owner: &RefIdMsg,
    owner_results: &mut ResultsMsg,
    other: &RefIdMsg,
    other_results: &ResultsMsg,
    update_type: &reference_msg::UpdateType,
) {
    if let Some(pt) = get_profile_pt_to_update(owner, owner_results) {
        if let Some(other_pt) = get_pt_from_other(other, other_results, update_type) {
            *pt = other_pt.clone();
        }
    }
}

fn delete_ref(owner_obj: &mut ObjectMsg, refer: &ReferenceMsg) {
    if let Some(deps) = &mut owner_obj.dependencies {
        for refer_opt in &mut deps.references {
            let mut delete = false;
            if let Some(existing_ref) = &mut refer_opt.reference {
                if *existing_ref == *refer {
                    delete = true;
                }
            }
            if delete {
                refer_opt.reference = None;
            }
        }
    }
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

pub async fn update_changes(
    obj_client: &mut ObjClient,
    dep_client: &mut DepClient,
    file: String,
    user: String,
    offset: i64,
    changes: Vec<ChangeMsg>,
) -> Result<Vec<ChangeMsg>, tonic::Status> {
    let (ref_ids, mut objects) = extract_info(changes);
    let refers = get_all_dependencies(dep_client, &file, offset, ref_ids).await?;
    let obj_ids = get_obj_ids_to_fetch(&refers, &objects);
    get_objects_to_update(obj_client, &file, offset, user, obj_ids, &mut objects).await?;
    update(&refers, &mut objects);
    let results = objects.drain().map(|(_, value)| value).collect();
    Ok(results)
}
