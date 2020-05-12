use crate::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisibilityGroup {
    id: ObjID,
    children: Vec<Option<RefID>>,
}

impl VisibilityGroup {
    pub fn new() -> VisibilityGroup {
        let id = ObjID::new_v4();
        VisibilityGroup {
            id,
            children: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
#[typetag::serde]
impl Data for VisibilityGroup {
    fn get_id(&self) -> &ObjID {
        &self.id
    }

    fn reset_id(&mut self) {
        self.id = ObjID::new_v4();
    }

    fn clear_refs(&mut self) {
        self.children.clear();
    }

    fn get_refs(&self) -> Vec<Option<Reference>> {
        let mut results = Vec::new();
        let mut index = 0;
        for child_opt in &self.children {
            if let Some(child) = child_opt {
                results.push(Some(Reference {
                    owner: RefID::new(self.id, RefType::Drawable, index),
                    other: *child,
                }));
            } else {
                results.push(None);
            }
            index += 1;
        }
        results
    }

    fn add_ref(
        &mut self,
        ref_type: RefType,
        _result: RefResult,
        other_ref: RefID,
        _extra: &Option<RefResult>,
    ) -> bool {
        if let RefType::Drawable = ref_type {
            self.children.push(Some(other_ref));
            true
        } else {
            false
        }
    }

    ///We can't delete things out of the vector without invalidating other refs, so just set it to None.
    /// This definitely is inefficient, but there's things we can do when we save to disk to fix this.
    fn delete_ref(&mut self, ref_type: RefType, index: ResultInd) {
        if let RefType::Drawable = ref_type {
            if let Some(child_opt) = self.children.get_mut(index) {
                *child_opt = None;
            }
        }
    }

    fn set_associated_result_for_type(
        &mut self,
        ref_type: RefType,
        index: ResultInd,
        result: Option<RefResult>,
    ) {
        if let RefType::Drawable = ref_type {
            if let Some(child_opt) = self.children.get_mut(index) {
                if let None = result {
                    *child_opt = None;
                }
            }
        }
    }

    fn data_clone(&self) -> DataBox {
        Box::new(self.clone())
    }
}
