mod object_state {
    include!(concat!(env!("OUT_DIR"), "/object_state.rs"));
    impl OptionPoint3Msg {
        pub fn new(pt: Option<geom::Point3Msg>) -> OptionPoint3Msg {
            OptionPoint3Msg { pt }
        }
    }

    impl ObjectMsg {
        pub fn get_profile_pt(&self, index: usize) -> Option<&geom::Point3Msg> {
            let mut result = None;
            if let Some(results) = &self.results {
                if let Some(profile) = &results.profile {
                    if let Some(pt_opt) = profile.points.get(index) {
                        if let Some(pt) = &pt_opt.pt {
                            result = Some(pt);
                        }
                    }
                }
            }
            result
        }

        pub fn get_profile_line(&self, index: usize) -> Option<&geom::LineMsg> {
            let mut result = None;
            if let Some(results) = &self.results {
                if let Some(profile) = &results.profile {
                    if let Some(line_opt) = profile.lines.get(index) {
                        if let Some(line) = &line_opt.line {
                            result = Some(line);
                        }
                    }
                }
            }
            result
        }

        pub fn get_profile_plane(&self, index: usize) -> Option<&geom::PlaneMsg> {
            let mut result = None;
            if let Some(results) = &self.results {
                if let Some(profile) = &results.profile {
                    if let Some(plane_opt) = profile.planes.get(index) {
                        if let Some(plane) = &plane_opt.plane {
                            result = Some(plane);
                        }
                    }
                }
            }
            result
        }

        pub fn get_properties(&self) -> Result<Option<serde_json::Value>, serde_json::Error> {
            let mut result = None;
            if let Some(results) = &self.results {
                if let Some(prop_msg) = &results.properties {
                    let json = serde_json::from_str(&prop_msg.prop_json)?;
                    result = Some(json);
                }
            }
            Ok(result)
        }

        pub fn get_reference(&self, index: usize) -> Option<&ReferenceMsg> {
            let mut result = None;
            if let Some(deps) = &self.dependencies {
                if let Some(ref_opt) = deps.references.get(index) {
                    if let Some(refer) = &ref_opt.reference {
                        result = Some(refer);
                    }
                }
            }
            result
        }

        pub fn get_bbox(&self) -> Option<&AxisAlignedBBoxMsg> {
            let mut result = None;
            if let Some(results) = &self.results {
                if let Some(bbox) = &results.bbox {
                    result = Some(bbox)
                }
            }
            result
        }

        pub fn get_bbox_mut(&mut self) -> Option<&mut AxisAlignedBBoxMsg> {
            let mut result = None;
            if let Some(results) = &mut self.results {
                if let Some(bbox) = &mut results.bbox {
                    result = Some(bbox)
                }
            }
            result
        }
    }
}

pub use object_state::*;
