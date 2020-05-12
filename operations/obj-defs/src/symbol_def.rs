use crate::*;
use serde::{Deserialize, Serialize};

///Symbols are objects that are defined in a separate symbol file.  The contents of a symbol are not referenceable outside of their file, so
/// we only reference the overall bounding box.  The Symbol Definition in the main file holds that bounding box.  
/// When the symbol file changes, it holds the address of every symbol definition referencing it.  It goes through and updates those symbol definitions using set_bbox.
/// Symbol instances in the main file reference this Symbol Definition, and so will be changed when this changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolDef {
    id: ObjID,
    sym_file: FileID,
    change: ChangeID,
    ///This is the axis-aligned bounding box of stuff that's defined in another file.  The bottom_left is always at (0, 0, 0).
    /// Individual symbol instances will transform this bounding box to their required positions and orientations
    bbox: Cube,
}

impl SymbolDef {
    pub fn new(sym_file: FileID, change: ChangeID, bbox: Cube) -> SymbolDef {
        let id = ObjID::new_v4();
        SymbolDef {
            id,
            sym_file,
            change,
            bbox,
        }
    }

    pub fn set_bbox(&mut self, sym_file: FileID, change: ChangeID, bbox: Cube) {
        self.sym_file = sym_file;
        self.change = change;
        self.bbox = bbox;
    }
}

#[async_trait::async_trait]
#[typetag::serde]
impl Data for SymbolDef {
    fn get_id(&self) -> &ObjID {
        &self.id
    }

    fn reset_id(&mut self) {
        self.id = ObjID::new_v4();
    }

    async fn update(&self, _conn: &mut dyn GeomKernel) -> Result<UpdateOutput, ObjError> {
        Ok(UpdateOutput::FileRef {
            file: self.sym_file,
        })
    }

    fn get_result(&self, ref_type: RefType, _result: ResultInd) -> Option<RefResult> {
        match ref_type {
            RefType::Existence => Some(RefResult::Empty),
            RefType::AxisAlignedBoundBox => Some(self.bbox.as_result()),
            _ => None,
        }
    }

    fn get_results_for_type(&self, ref_type: RefType) -> Vec<RefResult> {
        match ref_type {
            RefType::Existence => vec![RefResult::Empty],
            RefType::AxisAlignedBoundBox => vec![self.bbox.as_result()],
            _ => Vec::new(),
        }
    }

    fn get_num_results_for_type(&self, ref_type: RefType) -> usize {
        match ref_type {
            RefType::Existence => 1,
            RefType::AxisAlignedBoundBox => 1,
            _ => 0,
        }
    }

    fn data_clone(&self) -> DataBox {
        Box::new(self.clone())
    }
}
