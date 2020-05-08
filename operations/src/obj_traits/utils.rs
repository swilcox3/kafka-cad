use crate::*;

pub fn to_pt3f(pt_opt: &Option<Point3Msg>) -> Result<Point3f, OpsError> {
    match pt_opt {
        Some(pt) => Ok(Point3f::new(pt.x, pt.y, pt.z)),
        None => Err(OpsError::InvalidArgs),
    }
}

pub fn into_pt3f(pt_opt: Option<&Point3Msg>) -> Result<Point3f, OpsError> {
    match pt_opt {
        Some(pt) => Ok(Point3f::new(pt.x, pt.y, pt.z)),
        None => Err(OpsError::InvalidArgs),
    }
}

pub fn to_pt_msg(pt: &Point3f) -> Option<Point3Msg> {
    Some(Point3Msg {
        x: pt.x,
        y: pt.y,
        z: pt.z,
    })
}

pub fn bbox(
    pt_1: &Point3f,
    pt_2: &Point3f,
    width: WorldCoord,
    height: WorldCoord,
) -> Option<AxisAlignedBBoxMsg> {
    let bbox = get_axis_aligned_bound_box(pt_1, pt_2, width, height);
    Some(AxisAlignedBBoxMsg {
        bottom_left: to_pt_msg(&bbox.bottom_left),
        top_right: to_pt_msg(&bbox.top_right),
    })
}

pub fn props<T: serde::de::DeserializeOwned>(obj: &ObjectMsg) -> Result<T, OpsError> {
    if let Some(json) = obj.get_properties()? {
        let props = serde_json::from_value(json)?;
        Ok(props)
    } else {
        Err(OpsError::InvalidArgs)
    }
}
