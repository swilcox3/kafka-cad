//! The reference system at large works by addressing parts of an object.  One part of one object can subsscribe to updates to a part of another object.
//! By keeping track of what information references something else, we build up a graph of all the dependencies in the document.
use crate::*;
use cgmath::prelude::*;
use enum_iterator::IntoEnumIterator;

///The kind of information a given piece of information pertains to
#[derive(
    Debug, Copy, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Hash, IntoEnumIterator,
)]
pub enum RefType {
    Drawable, //Whether this object can be drawn in the model view or not.
    Existence,
    AxisAlignedBoundBox,
    ProfilePoint,
    ProfileLine,
    ProfilePlane,
    Property,
    Empty,
}

impl std::fmt::Display for RefType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefType::Drawable => write!(f, "Drawable"),
            RefType::Existence => write!(f, "Existence"),
            RefType::AxisAlignedBoundBox => write!(f, "AxisAlignedBoundBox"),
            RefType::ProfilePoint => write!(f, "ProfilePoint"),
            RefType::ProfileLine => write!(f, "ProfileLine"),
            RefType::ProfilePlane => write!(f, "ProfilePlane"),
            RefType::Property => write!(f, "Property"),
            RefType::Empty => write!(f, "Empty"),
        }
    }
}

pub fn str_to_ref_type(ref_str: &str) -> RefType {
    match ref_str {
        "Drawable" => RefType::Drawable,
        "Existence" => RefType::Existence,
        "AxisAlignedBoundBox" => RefType::AxisAlignedBoundBox,
        "ProfilePoint" => RefType::ProfilePoint,
        "ProfileLine" => RefType::ProfileLine,
        "ProfilePlane" => RefType::ProfilePlane,
        "Property" => RefType::Property,
        _ => RefType::Empty,
    }
}

/// Objects return vectors of information for each RefType.  This indexes into those vectors.
pub type ResultInd = usize;

///A part of an object.  This can be infoetry, a property, or a general quality of the object itself.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Hash)]
pub struct RefID {
    /// The object the information resides on.
    pub id: ObjID,
    /// What kind of information this RefID is pointing to
    pub ref_type: RefType,
    /// This indexes into the results an object has for the corresponding RefType
    pub index: ResultInd,
}

impl RefID {
    pub fn new(id: ObjID, ref_type: RefType, index: usize) -> RefID {
        RefID {
            id,
            ref_type,
            index,
        }
    }
}

impl std::fmt::Display for RefID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}>{:?}>{}", self.id, self.ref_type, self.index)
    }
}

///This represents a link between objects.  The owner is subscribed to changes made to other.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd, Eq, Hash)]
pub struct Reference {
    pub owner: RefID,
    pub other: RefID,
}

impl Reference {
    pub fn new(owner: RefID, other: RefID) -> Reference {
        Reference { owner, other }
    }
}

///Specific information that can be referenced on the object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RefResult {
    Empty,
    Point(Point3f),
    Line(Line),
    Plane(Plane),
    Cube(Cube),
    Property(serde_json::Value),
}

impl RefResult {
    //Only applies to geometrical RefResults
    pub fn distance2(&self, in_pt: &Point3f) -> Option<WorldCoord> {
        match self {
            RefResult::Point(pt) => Some(pt.distance2(*in_pt)),
            RefResult::Line(line) => {
                let projected = project_on_line(&line.pt_1, &line.pt_2, in_pt);
                Some(projected.distance2(*in_pt))
            }
            RefResult::Plane(plane) => Some(plane.pt_1.distance2(*in_pt)),
            RefResult::Cube(cube) => Some(cube.bottom_left.distance2(*in_pt)),
            RefResult::Property(..) | RefResult::Empty => None,
        }
    }
}

pub trait AsRefResult: Sized {
    fn as_result(&self) -> RefResult;
    fn from_result(result: RefResult) -> Option<Self>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UpdatableInfo<T: AsRefResult> {
    pub refer: Option<RefID>,
    pub info: T,
}

impl<T: AsRefResult> UpdatableInfo<T> {
    pub fn new(info: T) -> UpdatableInfo<T> {
        UpdatableInfo { refer: None, info }
    }

    pub fn update(&mut self, ref_result: Option<RefResult>) {
        if let Some(result) = ref_result {
            if let Some(info) = T::from_result(result) {
                self.info = info;
            }
        } else {
            self.refer = None;
        }
    }

    pub fn set_reference(&mut self, result: RefResult, refer: RefID) {
        if let Some(info) = T::from_result(result) {
            self.refer = Some(refer);
            self.info = info;
        }
    }

    pub fn get_result(&self) -> RefResult {
        self.info.as_result()
    }
}
