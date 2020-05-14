use crate::*;
use indexmap::IndexMap;

fn get_ref_result(
    objs: &IndexMap<ObjID, Option<DataBox>>,
    index: &RefID,
) -> Option<RefResult> {
    match objs.get(&index.id) {
        Some(obj_opt) => match obj_opt {
            Some(obj) => obj.get_result(index.ref_type, index.index),
            None => None,
        },
        None => None,
    }
}

fn update_reference(objs: &mut IndexMap<ObjID, Option<DataBox>>, refer: &Reference) {
    if refer.owner.id != refer.other.id {
        let result = get_ref_result(&objs, &refer.other);
        if let Some(updatable_opt) = objs.get_mut(&refer.owner.id) {
            if let Some(updatable) = updatable_opt {
                updatable.set_associated_result_for_type(refer.owner.ref_type, refer.owner.index, result);
            }
        }
    }
}

pub fn update_all(
    objs: &mut IndexMap<ObjID, Option<DataBox>>,
    refs: Vec<Reference>,
) {
    for refer in refs {
        update_reference(objs, &refer);
    }
}
