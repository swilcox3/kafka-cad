use bitflags::bitflags;
use enum_iterator::IntoEnumIterator;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

mod geom;
mod references;

pub use async_trait;
pub use cgmath;
pub use geom::*;
pub use prost;
pub use references::*;
pub use serde;
pub use serde_json;
pub use typetag;

pub type ObjID = Uuid;
pub type UserID = Uuid;
pub type OperationID = Uuid;
pub type FileID = Uuid;
pub type UndoEventID = Uuid;
pub type ChangeID = u64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UpdateOutput {
    Empty,
    Delete,
    Mesh { data: MeshData },
    FileRef { file: FileID },
    Instance { data: InstanceData },
    Other { data: serde_json::Value },
}

#[derive(Debug, Error)]
pub enum ObjError {
    #[error("Object {0:?} not found")]
    ObjNotFound(ObjID),
    #[error("Object {0:?} deleted")]
    ObjDeleted(ObjID),
    #[error("Object {0:?} is incorrect type, expected {1:?}")]
    ObjWrongType(ObjID, String),
    #[error("Geometry {0:?} not found")]
    GeomNotFound(RefID),
    #[error("Join could not be performed: {0}")]
    Join(String),
    #[error("Object {0:?} lacks trait {1}")]
    ObjLacksTrait(ObjID, String),
    #[error("JSON error")]
    JsonError(#[from] serde_json::Error),
    #[error("Bincode error")]
    BincodeError(#[from] bincode::Error),
    #[error("Prost encode error")]
    ProstEncodeError(#[from] prost::EncodeError),
    #[error("Prost decode error")]
    ProstDecodeError(#[from] prost::DecodeError),
    #[error("Couldn't connect to service: {0:?}")]
    ConnectError(#[from] tonic::transport::Error),
    #[error("{0}")]
    StatusError(#[from] tonic::Status),
    #[error("Uuid error")]
    UuidError(#[from] uuid::Error),
    #[error("Unknown error {0}")]
    Other(String),
}

impl Into<tonic::Status> for ObjError {
    fn into(self) -> tonic::Status {
        let msg = format!("{}", self);
        let code = match self {
            ObjError::BincodeError(..)
            | ObjError::JsonError(..)
            | ObjError::ProstEncodeError(..)
            | ObjError::ProstDecodeError(..)
            | ObjError::UuidError(..)
            | ObjError::Other(..) => tonic::Code::Internal,
            ObjError::ObjNotFound(..)
            | ObjError::ObjWrongType(..)
            | ObjError::ObjDeleted(..)
            | ObjError::GeomNotFound { .. }
            | ObjError::Join { .. } => tonic::Code::NotFound,
            ObjError::ObjLacksTrait(..) => tonic::Code::InvalidArgument,
            ObjError::ConnectError(..) => tonic::Code::Unavailable,
            ObjError::StatusError(status) => status.code(),
        };
        tonic::Status::new(code, msg)
    }
}

pub fn to_status<T: Into<ObjError>>(err: T) -> tonic::Status {
    let obj_error: ObjError = err.into();
    obj_error.into()
}

pub fn other_error<T: std::fmt::Debug>(err: T) -> ObjError {
    ObjError::Other(format!("{:?}", err))
}

#[async_trait::async_trait]
pub trait GeomKernel: Send + Sync {
    async fn make_prism(
        &mut self,
        first_pt: &Point3f,
        second_pt: &Point3f,
        width: f64,
        height: f64,
        result: &mut MeshData,
    ) -> Result<(), ObjError>;
}

#[derive(Debug)]
pub struct RefTypedResults {
    pub ref_type: RefType,
    pub results: Vec<RefResult>,
}

impl RefTypedResults {
    pub fn new(ref_type: RefType, results: Vec<RefResult>) -> RefTypedResults {
        RefTypedResults { ref_type, results }
    }
}

pub trait Position {
    fn move_obj(&mut self, delta: &Vector3f);
    fn get_axis_aligned_bounding_box(&self) -> Cube;
}

bitflags! {
    ///Defines what views to generate for an object
    pub struct ViewFlags: u32 {
        const TOP = 0b00000001;
        const FRONT = 0b00000010;
        const LEFT = 0b00000100;
        const RIGHT = 0b00001000;
        const BACK = 0b00010000;
        const BOTTOM = 0b00100000;
    }
}

pub trait DrawingViews {
    fn get_top(&self) -> DrawingData;
    fn get_front(&self) -> DrawingData;
    fn get_left(&self) -> DrawingData;
    fn get_right(&self) -> DrawingData;
    fn get_back(&self) -> DrawingData;
    fn get_bottom(&self) -> DrawingData;
    fn get_views(&self, flags: ViewFlags) -> DrawingRepresentations {
        let top = if flags.contains(ViewFlags::TOP) {
            Some(self.get_top())
        } else {
            None
        };
        let front = if flags.contains(ViewFlags::FRONT) {
            Some(self.get_front())
        } else {
            None
        };
        let left = if flags.contains(ViewFlags::LEFT) {
            Some(self.get_left())
        } else {
            None
        };
        let right = if flags.contains(ViewFlags::RIGHT) {
            Some(self.get_right())
        } else {
            None
        };
        let back = if flags.contains(ViewFlags::BACK) {
            Some(self.get_back())
        } else {
            None
        };
        let bottom = if flags.contains(ViewFlags::BOTTOM) {
            Some(self.get_bottom())
        } else {
            None
        };
        DrawingRepresentations {
            top,
            front,
            left,
            right,
            back,
            bottom,
        }
    }
}

///The basic trait that all objects must implement.  Defaults to doing nothing for most functions.
/// This way, if your object doesn't have results or references, you don't have to implement those functions.
/// Most objects will need to implement all of these.
#[allow(unused_variables)]
#[async_trait::async_trait]
#[typetag::serde(tag = "type")]
pub trait Data: std::fmt::Debug + Send + Sync + downcast_rs::Downcast {
    fn get_id(&self) -> &ObjID;
    ///Used when copy/pasting an object, we need a new ID for the copied object
    fn reset_id(&mut self);
    async fn update(&self, conn: &mut dyn GeomKernel) -> Result<UpdateOutput, ObjError> {
        Ok(UpdateOutput::Empty)
    }

    fn get_result(&self, ref_type: RefType, index: ResultInd) -> Option<RefResult> {
        None
    }

    fn get_results_for_type(&self, ref_type: RefType) -> Vec<RefResult> {
        Vec::new()
    }

    fn get_all_results(&self) -> Vec<RefTypedResults> {
        let mut results = Vec::new();
        for ref_type in RefType::into_enum_iter() {
            if self.get_num_results_for_type(ref_type) > 0 {
                results.push(RefTypedResults::new(
                    ref_type,
                    self.get_results_for_type(ref_type),
                ));
            }
        }
        results
    }

    fn get_num_results_for_type(&self, ref_type: RefType) -> usize {
        0
    }

    fn clear_refs(&mut self) {}

    fn get_refs(&self) -> Vec<Option<Reference>> {
        Vec::new()
    }

    fn get_available_refs_for_type(&self, ref_type: RefType) -> Vec<ResultInd> {
        Vec::new()
    }

    fn set_ref(
        &mut self,
        ref_type: RefType,
        index: ResultInd,
        result: RefResult,
        other_ref: RefID,
        extra: &Option<RefResult>,
    ) {
    }

    fn add_ref(
        &mut self,
        ref_type: RefType,
        result: RefResult,
        other_ref: RefID,
        extra: &Option<RefResult>,
    ) -> bool {
        false
    }

    fn delete_ref(&mut self, ref_type: RefType, index: ResultInd) {}

    fn set_associated_result_for_type(
        &mut self,
        ref_type: RefType,
        index: ResultInd,
        result: Option<RefResult>,
    ) {
    }

    //This one must be implemented, it allows us to clone without downcasting
    fn data_clone(&self) -> DataBox;

    fn as_position(&self) -> Option<&dyn Position> {
        None
    }
    fn as_position_mut(&mut self) -> Option<&mut dyn Position> {
        None
    }

    fn as_drawing_views(&self) -> Option<&dyn DrawingViews> {
        None
    }
}
downcast_rs::impl_downcast!(Data);

pub type DataBox = Box<dyn Data>;
