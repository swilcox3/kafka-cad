use crate::*;

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
        references: ref_msgs,
        obj_data: bincode::serialize(obj)?,
    })
}
