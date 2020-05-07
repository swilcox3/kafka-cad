use crate::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct WallProps {
    #[serde(rename = "Width")]
    pub width: f64,
    #[serde(rename = "Height")]
    pub height: f64,
}

pub struct Wall {
    pub first_pt: Point3Msg,
    pub second_pt: Point3Msg,
    pub width: f64,
    pub height: f64,
}

impl Wall {
    pub fn from_obj_msg(msg: &ObjectMsg) -> Result<Wall, WallError> {
        let mut result = None;
        if let Some(first_pt) = msg.get_profile_pt(0) {
            if let Some(second_pt) = msg.get_profile_pt(1) {
                if let Some(props_json) = msg.get_properties()? {
                    let props: WallProps = serde_json::from_value(props_json)?;
                    result = Some(Wall {
                        first_pt: first_pt.clone(),
                        second_pt: second_pt.clone(),
                        width: props.width,
                        height: props.height,
                    });
                }
            }
        }
        match result {
            Some(wall) => Ok(wall),
            None => Err(WallError::InvalidArgs),
        }
    }

    pub fn get_props(&self) -> WallProps {
        WallProps {
            width: self.width,
            height: self.height,
        }
    }
}
