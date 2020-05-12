use crate::*;

async fn get_closest_result(
    obj: &DataBox,
    only_match: RefType,
    guess: &Point3f,
) -> Result<Option<(RefID, RefResult)>, ObjError> {
    let mut result = None;
    let results = obj.get_results_for_type(only_match);
    let mut dist = std::f64::MAX;
    let mut index = 0;
    for ref_res in results {
        if let Some(cur_dist) = ref_res.distance2(&guess) {
            if cur_dist < dist {
                let which = RefID {
                    id: *obj.get_id(),
                    ref_type: only_match,
                    index,
                };
                result = Some((which, ref_res));
                dist = cur_dist;
            }
        }
        index += 1;
    }
    Ok(result)
}

async fn get_closest_ref(
    obj: &DataBox,
    only_match: RefType,
    guess: &Point3f,
) -> Result<Option<ResultInd>, ObjError> {
    let mut result_ind = None;
    let indices = obj.get_available_refs_for_type(only_match);
    let mut dist = std::f64::MAX;
    for index in indices {
        if let Some(ref_result) = obj.get_result(only_match, index) {
            trace!(
                "Looking at ref_type {:?}, ref_result {:?} for {:?}",
                only_match,
                ref_result,
                obj.get_id()
            );
            if let Some(cur_dist) = ref_result.distance2(guess) {
                if cur_dist < dist {
                    result_ind = Some(index);
                    dist = cur_dist;
                }
            }
        }
    }
    Ok(result_ind)
}

pub async fn snap_to_ref(
    obj: &mut DataBox,
    other_obj: &DataBox,
    only_match: RefType,
    guess: &Point3f,
) -> Result<(), ObjError> {
    let res_opt = get_closest_result(other_obj, only_match, guess).await?;
    if let Some((which, calc_res)) = res_opt {
        trace!(
            "Looking at which {:?}, calc_res {:?} from {:?}",
            which,
            calc_res,
            other_obj.get_id()
        );
        let which_opt = get_closest_ref(obj, only_match, guess).await?;
        trace!("which_opt {:?}", which_opt);
        match which_opt {
            Some(index) => obj.set_ref(only_match, index, calc_res, which, &Some(RefResult::Point(*guess))),
            None => {
                if !obj.add_ref(only_match, calc_res, which, &Some(RefResult::Point(*guess))) {
                    return Err(ObjError::Join(format!("Couldn't add ref to {}", obj.get_id())));
                }
            }
        }
        Ok(())
    } else {
        Err(ObjError::Join(String::from("Nothing to snap to")))
    }
}

pub async fn join_refs(
    first: &mut DataBox,
    second: &mut DataBox,
    first_wants: RefType,
    second_wants: RefType,
    guess: &Point3f,
) -> Result<(), ObjError> {
    snap_to_ref(second, first, second_wants, guess).await?;
    snap_to_ref(first, second, first_wants, guess).await?;
    Ok(())
}
