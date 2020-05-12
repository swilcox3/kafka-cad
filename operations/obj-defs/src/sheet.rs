use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sheet {
    id: ObjID,
    pub print_size: Point2f,
}

impl Sheet {
    pub fn new(print_size: Point2f) -> Sheet {
        let id = ObjID::new_v4();
        Sheet { id, print_size }
    }
}

#[async_trait::async_trait]
#[typetag::serde]
impl Data for Sheet {
    fn get_id(&self) -> &ObjID {
        &self.id
    }

    fn reset_id(&mut self) {
        self.id = ObjID::new_v4();
    }

    async fn update(&self, _conn: &mut dyn GeomKernel) -> Result<UpdateOutput, ObjError> {
        Ok(UpdateOutput::Other {
            data: json! ({
                "type": "Sheet",
                "obj": {
                    "print_size": self.print_size
                }
            }),
        })
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

    fn data_clone(&self) -> DataBox {
        Box::new(self.clone())
    }
}
