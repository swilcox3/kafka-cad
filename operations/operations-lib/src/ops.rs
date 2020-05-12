use crate::*;

pub async fn move_objects(
    mut objs: Vec<DataBox>,
    delta: &Vector3f,
) -> Result<Vec<DataBox>, ObjError> {
    for obj in &mut objs {
        match obj.as_position_mut() {
            Some(pos) => {
                pos.move_obj(delta);
            }
            None => {
                return Err(ObjError::ObjLacksTrait(
                    obj.get_id().clone(),
                    String::from("Position"),
                ))
            }
        }
    }
    Ok(objs)
}

pub async fn add_objs_to_visibility_group(
    group: &mut DataBox,
    objs: &Vec<DataBox>,
) -> Result<(), ObjError> {
    //Make sure it's a visibility group
    if let None = group.downcast_ref::<VisibilityGroup>() {
        return Err(ObjError::ObjWrongType(
            *group.get_id(),
            String::from("VisibilityGroup"),
        ));
    }
    for obj in objs {
        if let Some(res) = obj.get_result(RefType::Drawable, 0) {
            group.add_ref(
                RefType::Drawable,
                res,
                RefID::new(*obj.get_id(), RefType::Drawable, 0),
                &None,
            );
        }
    }
    Ok(())
}

pub async fn remove_objs_from_visibility_group(
    group: &mut DataBox,
    objs: &Vec<ObjID>,
) -> Result<(), ObjError> {
    //Make sure it's a visibility group
    if let None = group.downcast_ref::<VisibilityGroup>() {
        return Err(ObjError::ObjWrongType(
            *group.get_id(),
            String::from("VisibilityGroup"),
        ));
    }
    for ref_opt in group.get_refs() {
        if let Some(refer) = ref_opt {
            for obj_id in objs {
                if refer.other.id == *obj_id {
                    group.delete_ref(RefType::Drawable, refer.owner.index);
                    break;
                }
            }
        }
    }
    Ok(())
}
