use crate::*;
use operations::indexmap::IndexMap;

pub fn to_obj_id(id: &str) -> Result<ObjID, tonic::Status> {
    let parsed =
        ObjID::parse_str(id).map_err(|e| tonic::Status::invalid_argument(format!("{:?}", e)))?;
    Ok(parsed)
}

pub fn to_point_2u(msg: &Option<Point2Msg>) -> Result<Point2f, tonic::Status> {
    if let Some(pt_msg) = msg {
        Ok(Point2f::new(pt_msg.x, pt_msg.y))
    } else {
        Err(tonic::Status::invalid_argument("No point passed in"))
    }
}

pub fn to_point_3f(msg: &Option<Point3Msg>) -> Result<Point3f, tonic::Status> {
    if let Some(pt_msg) = msg {
        Ok(Point3f::new(pt_msg.x, pt_msg.y, pt_msg.z))
    } else {
        Err(tonic::Status::invalid_argument("No point passed in"))
    }
}

pub fn to_door(
    first_pt: &Option<Point3Msg>,
    second_pt: &Option<Point3Msg>,
    width: WorldCoord,
    height: WorldCoord,
) -> Result<Door, tonic::Status> {
    Ok(Door::new(
        to_point_3f(first_pt)?,
        to_point_3f(second_pt)?,
        width,
        height,
    ))
}

pub fn to_wall(
    first_pt: &Option<Point3Msg>,
    second_pt: &Option<Point3Msg>,
    width: WorldCoord,
    height: WorldCoord,
) -> Result<Wall, tonic::Status> {
    Ok(Wall::new(
        to_point_3f(first_pt)?,
        to_point_3f(second_pt)?,
        width,
        height,
    ))
}

pub fn get_view_type(view_msg: &str) -> Result<ViewType, tonic::Status> {
    match serde_json::from_str(view_msg) {
        Ok(view) => Ok(view),
        Err(e) => Err(tonic::Status::invalid_argument(format!(
            "Invalid json for view type: {:?}",
            e
        ))),
    }
}

pub fn from_object_msg(msg: &ObjectMsg) -> Result<DataBox, ObjError> {
    let obj: DataBox = bincode::deserialize(&msg.obj_data)?;
    Ok(obj)
}

pub fn from_ref_msgs(msgs: &Vec<ReferenceMsg>) -> Result<Vec<Reference>, tonic::Status> {
    let mut results = Vec::new();
    for msg in msgs {
        results.push(Reference {
            owner: from_ref_id_msg(&msg.owner)?,
            other: from_ref_id_msg(&msg.other)?,
        });
    }
    Ok(results)
}

fn from_ref_id_msg(msg: &Option<RefIdMsg>) -> Result<RefID, tonic::Status> {
    match msg {
        Some(msg) => Ok(RefID {
            id: to_obj_id(&msg.id)?,
            ref_type: match ref_id_msg::RefType::from_i32(msg.ref_type) {
                Some(ref_id_msg::RefType::Drawable) => RefType::Drawable,
                Some(ref_id_msg::RefType::Existence) => RefType::Existence,
                Some(ref_id_msg::RefType::AxisAlignedBbox) => RefType::AxisAlignedBoundBox,
                Some(ref_id_msg::RefType::ProfilePoint) => RefType::ProfilePoint,
                Some(ref_id_msg::RefType::ProfileLine) => RefType::ProfileLine,
                Some(ref_id_msg::RefType::ProfilePlane) => RefType::ProfilePlane,
                Some(ref_id_msg::RefType::Property) => RefType::Property,
                Some(ref_id_msg::RefType::Empty) => RefType::Empty,
                None => return Err(tonic::Status::invalid_argument("No ref type set")),
            },
            index: msg.index as ResultInd,
        }),
        None => Err(tonic::Status::invalid_argument("No ref id passed in")),
    }
}

fn to_ref_id_msg(ref_id: &RefID) -> RefIdMsg {
    RefIdMsg {
        id: ref_id.id.to_string(),
        ref_type: match ref_id.ref_type {
            RefType::Drawable => ref_id_msg::RefType::Drawable as i32,
            RefType::Existence => ref_id_msg::RefType::Existence as i32,
            RefType::AxisAlignedBoundBox => ref_id_msg::RefType::AxisAlignedBbox as i32,
            RefType::ProfilePoint => ref_id_msg::RefType::ProfilePoint as i32,
            RefType::ProfileLine => ref_id_msg::RefType::ProfileLine as i32,
            RefType::ProfilePlane => ref_id_msg::RefType::ProfilePlane as i32,
            RefType::Property => ref_id_msg::RefType::Property as i32,
            RefType::Empty => ref_id_msg::RefType::Empty as i32,
        },
        index: ref_id.index as u64,
    }
}

pub fn to_object_msg(obj: &DataBox) -> Result<ObjectMsg, ObjError> {
    let refs = obj.get_refs();
    let mut ref_msgs = Vec::new();
    for refer_opt in refs {
        match refer_opt {
            Some(refer) => {
                ref_msgs.push(OptionReferenceMsg {
                    reference: Some(ReferenceMsg {
                        owner: Some(to_ref_id_msg(&refer.owner)),
                        other: Some(to_ref_id_msg(&refer.other)),
                    }),
                });
            }
            None => {
                ref_msgs.push(OptionReferenceMsg { reference: None });
            }
        }
    }
    Ok(ObjectMsg {
        id: obj.get_id().to_string(),
        dependencies: Some(DependenciesMsg {
            references: ref_msgs,
        }),
        obj_data: bincode::serialize(obj)?,
    })
}

pub fn from_change_msgs(
    msgs: &Vec<ChangeMsg>,
) -> Result<IndexMap<ObjID, Option<DataBox>>, tonic::Status> {
    let mut results = IndexMap::new();
    for msg in msgs {
        match &msg.change_type {
            Some(change_msg::ChangeType::Add(object))
            | Some(change_msg::ChangeType::Modify(object)) => {
                let id = to_obj_id(&object.id)?;
                results.insert(id, Some(from_object_msg(&object).map_err(to_status)?));
            }
            Some(change_msg::ChangeType::Delete(msg)) => {
                let id = to_obj_id(&msg.id)?;
                results.insert(id, None);
            }
            None => (),
        }
    }
    Ok(results)
}

pub fn to_change_msgs(
    old_changes: &Vec<ChangeMsg>,
    objects: &IndexMap<ObjID, Option<DataBox>>,
) -> Result<Vec<ChangeMsg>, ObjError> {
    let mut results = Vec::new();
    for i in 0..old_changes.len() {
        if let Some(old_change) = old_changes.get(i) {
            if let Some((id, obj_opt)) = objects.get_index(i) {
                match old_change.change_type {
                    Some(change_msg::ChangeType::Add(..)) => {
                        if let Some(obj) = obj_opt {
                            let change = ChangeMsg {
                                user: old_change.user.clone(),
                                change_type: Some(change_msg::ChangeType::Add(to_object_msg(obj)?)),
                            };
                            results.push(change);
                        }
                    }
                    Some(change_msg::ChangeType::Modify(..)) => {
                        if let Some(obj) = obj_opt {
                            let change = ChangeMsg {
                                user: old_change.user.clone(),
                                change_type: Some(change_msg::ChangeType::Modify(to_object_msg(
                                    obj,
                                )?)),
                            };
                            results.push(change);
                        }
                    }
                    Some(change_msg::ChangeType::Delete(..)) => {
                        let change = ChangeMsg {
                            user: old_change.user.clone(),
                            change_type: Some(change_msg::ChangeType::Delete(DeleteMsg {
                                id: id.to_string(),
                            })),
                        };
                        results.push(change);
                    }
                    None => {
                        results.push(ChangeMsg {
                            user: old_change.user.clone(),
                            change_type: None,
                        });
                    }
                }
            }
        }
    }
    Ok(results)
}
