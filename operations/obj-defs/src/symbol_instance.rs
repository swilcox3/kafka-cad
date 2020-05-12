use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

///Symbols are objects that are defined in a separate symbol file.  The contents of a symbol are not referenceable outside of their file, so
/// we only reference the overall bounding box.  The Symbol Definition in the main file holds that bounding box.  
/// When the symbol file changes, it holds the address of every symbol definition referencing it.  It goes through and updates those symbol definitions using set_bbox.
/// Symbol instances in the main file reference this Symbol Definition, and so will be changed when this changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolInstance {
    id: ObjID,
    bbox: UpdatableInfo<Cube>,
    transform: TransMat,
}

impl SymbolInstance {
    pub fn new() -> SymbolInstance {
        let id = ObjID::new_v4();
        SymbolInstance {
            id,
            bbox: UpdatableInfo::new(Cube::default()),
            transform: identity_mat(),
        }
    }

    fn get_transformed(&self) -> RefResult {
        let transformed = apply_transform(self.transform, self.bbox.info);
        RefResult::Cube(transformed)
    }
}

#[async_trait::async_trait]
#[typetag::serde]
impl Data for SymbolInstance {
    fn get_id(&self) -> &ObjID {
        &self.id
    }

    fn reset_id(&mut self) {
        self.id = ObjID::new_v4();
    }

    async fn update(&self, _conn: &mut dyn GeomKernel) -> Result<UpdateOutput, ObjError> {
        let source_id = match self.bbox.refer {
            Some(sym_def) => Some(sym_def.id),
            None => None,
        };
        let data = InstanceData {
            transform: self.transform,
            bbox: apply_transform(self.transform, self.bbox.info),
            source: source_id,
            metadata: Some(json! ({
                "type": "Wall",
                "traits": ["Position"],
            })),
        };
        Ok(UpdateOutput::Instance { data })
    }

    fn get_result(&self, ref_type: RefType, _result: ResultInd) -> Option<RefResult> {
        match ref_type {
            RefType::Drawable => Some(RefResult::Empty),
            RefType::Existence => Some(RefResult::Empty),
            RefType::AxisAlignedBoundBox => Some(self.get_transformed()),
            _ => None,
        }
    }

    fn get_results_for_type(&self, ref_type: RefType) -> Vec<RefResult> {
        match ref_type {
            RefType::Drawable => vec![RefResult::Empty],
            RefType::Existence => vec![RefResult::Empty],
            RefType::AxisAlignedBoundBox => vec![self.get_transformed()],
            _ => Vec::new(),
        }
    }

    fn get_num_results_for_type(&self, ref_type: RefType) -> usize {
        match ref_type {
            RefType::Drawable => 1,
            RefType::Existence => 1,
            RefType::AxisAlignedBoundBox => 1,
            _ => 0,
        }
    }

    fn clear_refs(&mut self) {
        self.bbox.refer = None;
    }

    fn get_refs(&self) -> Vec<Option<Reference>> {
        let mut refers = Vec::new();
        if let Some(refer) = self.bbox.refer {
            refers.push(Some(Reference {
                owner: RefID::new(self.id, RefType::AxisAlignedBoundBox, 0),
                other: refer.clone(),
            }));
        } else {
            refers.push(None);
        }
        refers
    }

    fn get_available_refs_for_type(&self, ref_type: RefType) -> Vec<ResultInd> {
        let mut results = Vec::new();
        if let RefType::AxisAlignedBoundBox = ref_type {
            if let None = self.bbox.refer {
                results.push(0);
            }
        }
        results
    }

    fn set_ref(
        &mut self,
        ref_type: RefType,
        index: ResultInd,
        result: RefResult,
        other_ref: RefID,
        _extra: &Option<RefResult>,
    ) {
        if let RefType::AxisAlignedBoundBox = ref_type {
            match index {
                0 => self.bbox.set_reference(result, other_ref),
                _ => (),
            }
        }
    }

    fn delete_ref(&mut self, ref_type: RefType, index: ResultInd) {
        if let RefType::AxisAlignedBoundBox = ref_type {
            match index {
                0 => self.bbox.refer = None,
                _ => (),
            }
        }
    }

    fn set_associated_result_for_type(
        &mut self,
        ref_type: RefType,
        index: ResultInd,
        result: Option<RefResult>,
    ) {
        if let RefType::AxisAlignedBoundBox = ref_type {
            match index {
                0 => self.bbox.update(result),
                _ => (),
            }
        }
    }

    fn as_position(&self) -> Option<&dyn Position> {
        Some(self)
    }
    fn as_position_mut(&mut self) -> Option<&mut dyn Position> {
        Some(self)
    }

    fn data_clone(&self) -> DataBox {
        Box::new(self.clone())
    }
}

impl Position for SymbolInstance {
    fn move_obj(&mut self, delta: &Vector3f) {
        self.transform
            .concat_self(&TransMat::from_translation(*delta));
    }

    fn get_axis_aligned_bounding_box(&self) -> Cube {
        apply_transform(self.transform, self.bbox.info)
    }
}
