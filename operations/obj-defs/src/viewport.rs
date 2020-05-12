use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ViewType {
    Top,
    Front,
    Left,
    Right,
    Back,
    Bottom,
    Custom {
        camera_pos: Vector3f,
        target: Vector3f,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Viewport {
    id: ObjID,
    pub view: ViewType,
    /// If this is None, the viewport will delete itself on update.
    sheet: Option<ObjID>,
    /// This is offset in pixels of the top left corner of the viewport from the top left corner of the sheet.
    pub origin: Point2f,
}

impl Viewport {
    pub fn new(sheet: ObjID, view: ViewType, origin: Point2f) -> Viewport {
        let id = ObjID::new_v4();
        Viewport {
            id,
            sheet: Some(sheet),
            view,
            origin,
        }
    }
}

#[async_trait::async_trait]
#[typetag::serde]
impl Data for Viewport {
    fn get_id(&self) -> &ObjID {
        &self.id
    }

    fn reset_id(&mut self) {
        self.id = ObjID::new_v4();
    }

    async fn update(&self, _conn: &mut dyn GeomKernel) -> Result<UpdateOutput, ObjError> {
        match self.sheet {
            Some(sheet_id) => Ok(UpdateOutput::Other {
                data: json! ({
                    "type": "Viewport",
                    "obj": {
                        "view": self.view,
                        "sheet": sheet_id.to_string(),
                        "origin": self.origin,
                    }
                }),
            }),
            None => Ok(UpdateOutput::Delete),
        }
    }

    fn get_result(&self, ref_type: RefType, _index: ResultInd) -> Option<RefResult> {
        match ref_type {
            RefType::Existence => Some(RefResult::Empty),
            _ => None,
        }
    }

    fn get_results_for_type(&self, ref_type: RefType) -> Vec<RefResult> {
        match ref_type {
            RefType::Existence => vec![RefResult::Empty],
            _ => Vec::new(),
        }
    }

    fn get_num_results_for_type(&self, ref_type: RefType) -> usize {
        match ref_type {
            RefType::Existence => 1,
            _ => 0,
        }
    }

    //This will delete the viewport on next update unless the sheet is reset.
    fn clear_refs(&mut self) {
        self.sheet = None;
    }

    fn get_refs(&self) -> Vec<Option<Reference>> {
        match self.sheet {
            Some(sheet_id) => vec![Some(Reference {
                owner: RefID::new(self.id, RefType::Existence, 0),
                other: RefID::new(sheet_id, RefType::Existence, 0),
            })],
            None => vec![None],
        }
    }

    fn set_ref(
        &mut self,
        ref_type: RefType,
        _index: ResultInd,
        _result: RefResult,
        other_ref: RefID,
        _extra: &Option<RefResult>,
    ) {
        match ref_type {
            RefType::Existence => self.sheet = Some(other_ref.id),
            _ => (),
        }
    }

    fn delete_ref(&mut self, ref_type: RefType, _index: ResultInd) {
        match ref_type {
            RefType::Existence => self.sheet = None,
            _ => (),
        }
    }

    fn data_clone(&self) -> DataBox {
        Box::new(self.clone())
    }
}
