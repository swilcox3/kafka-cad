use crate::*;
use cgmath::prelude::*;
pub use cgmath::Transform;
use serde::{Deserialize, Serialize};
use std::default::Default;

pub type Point2f = cgmath::Point2<f64>;
pub type Point3f = cgmath::Point3<f64>;
pub type WorldCoord = f64;
pub type Vector3f = cgmath::Vector3<f64>;
pub type TransMat = cgmath::Matrix4<f64>;
pub type Radians = cgmath::Rad<f64>;

pub fn radians(angle: f64) -> Radians {
    cgmath::Rad(angle)
}

impl AsRefResult for Point3f {
    fn as_result(&self) -> RefResult {
        RefResult::Point(*self)
    }
    fn from_result(result: RefResult) -> Option<Point3f> {
        if let RefResult::Point(pt) = result {
            Some(pt)
        } else {
            None
        }
    }
}

pub fn x_y(pt: &Point3f) -> Point2f {
    Point2f::new(pt.x, pt.y)
}

pub fn x_z(pt: &Point3f) -> Point2f {
    Point2f::new(pt.x, pt.z)
}

pub fn y_z(pt: &Point3f) -> Point2f {
    Point2f::new(pt.y, pt.z)
}

pub fn identity_mat() -> TransMat {
    TransMat::from_scale(1.0)
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Line {
    pub pt_1: Point3f,
    pub pt_2: Point3f,
}

impl Line {
    pub fn new(pt_1: Point3f, pt_2: Point3f) -> Line {
        Line { pt_1, pt_2 }
    }
}

impl Default for Line {
    fn default() -> Line {
        Line {
            pt_1: Point3f::new(0.0, 0.0, 0.0),
            pt_2: Point3f::new(0.0, 0.0, 0.0),
        }
    }
}

impl AsRefResult for Line {
    fn as_result(&self) -> RefResult {
        RefResult::Line(*self)
    }
    fn from_result(result: RefResult) -> Option<Line> {
        if let RefResult::Line(line) = result {
            Some(line)
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Plane {
    pub pt_1: Point3f,
    pub pt_2: Point3f,
    pub pt_3: Point3f,
}

impl Plane {
    pub fn new(pt_1: Point3f, pt_2: Point3f, pt_3: Point3f) -> Plane {
        Plane { pt_1, pt_2, pt_3 }
    }
}

impl Default for Plane {
    fn default() -> Plane {
        Plane {
            pt_1: Point3f::new(0.0, 0.0, 0.0),
            pt_2: Point3f::new(0.0, 0.0, 0.0),
            pt_3: Point3f::new(0.0, 0.0, 0.0),
        }
    }
}

impl AsRefResult for Plane {
    fn as_result(&self) -> RefResult {
        RefResult::Plane(*self)
    }
    fn from_result(result: RefResult) -> Option<Plane> {
        if let RefResult::Plane(plane) = result {
            Some(plane)
        } else {
            None
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cube {
    pub bottom_left: Point3f,
    pub top_right: Point3f,
}

impl Cube {
    pub fn new(bottom_left: Point3f, top_right: Point3f) -> Cube {
        Cube {
            bottom_left,
            top_right,
        }
    }
}

impl Default for Cube {
    fn default() -> Cube {
        Cube {
            bottom_left: Point3f::new(0.0, 0.0, 0.0),
            top_right: Point3f::new(0.0, 0.0, 0.0),
        }
    }
}

impl AsRefResult for Cube {
    fn as_result(&self) -> RefResult {
        RefResult::Cube(*self)
    }
    fn from_result(result: RefResult) -> Option<Cube> {
        if let RefResult::Cube(cube) = result {
            Some(cube)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RefLineSeg {
    pub refer: Option<RefID>,
    pub line: Line,
    pub length: WorldCoord,
    pub interp: Interp,
}

impl RefLineSeg {
    pub fn new(line: Line) -> RefLineSeg {
        let length = (line.pt_2 - line.pt_1).magnitude();
        RefLineSeg {
            refer: None,
            line,
            length: length,
            interp: Interp::new(0.0),
        }
    }

    pub fn set_dir(&mut self, dir: &Vector3f) {
        self.line.pt_2 = self.line.pt_1 + dir.normalize() * self.length;
    }

    pub fn update(&mut self, info: RefResult, snap_pt: &Option<RefResult>) {
        if let RefResult::Line(line) = info {
            if let Some(RefResult::Point(snap)) = snap_pt {
                self.interp = get_interp_along_line(&line.pt_1, &line.pt_2, snap);
            }
            let dir = line.pt_2 - line.pt_1;
            let norm = dir.normalize();
            self.line.pt_1 = line.pt_1 + dir * self.interp.val();
            self.line.pt_2 = self.line.pt_1 + norm * self.length;
        }
    }

    pub fn set_reference(&mut self, result: RefResult, refer: RefID, snap_pt: &Option<RefResult>) {
        if let RefResult::Line(_) = result {
            self.refer = Some(refer);
            self.update(result, snap_pt);
        }
    }

    pub fn get_result(&self) -> RefResult {
        self.line.as_result()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl RGBA {
    pub fn new(r: u8, g: u8, b: u8, a: f32) -> RGBA {
        RGBA { r, g, b, a }
    }
}

impl Default for RGBA {
    fn default() -> RGBA {
        //Opaque black
        RGBA {
            r: 0,
            g: 0,
            b: 0,
            a: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FillType {
    Solid { color: RGBA },
    Hatch { name: String },
}

impl Default for FillType {
    fn default() -> FillType {
        //Default to solid white
        FillType::Solid {
            color: RGBA::new(255, 255, 255, 1.0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LineType {
    Solid,
    Dashed { name: String },
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Line2D {
    pub first: Point2f,
    pub second: Point2f,
}

impl Line2D {
    pub fn new(first: Point2f, second: Point2f) -> Line2D {
        Line2D { first, second }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Arc2D {
    pub center: Point2f,
    pub radius: WorldCoord,
    pub start_angle: Radians,
    pub end_angle: Radians,
}

impl Arc2D {
    pub fn new(
        center: Point2f,
        radius: WorldCoord,
        start_angle: Radians,
        end_angle: Radians,
    ) -> Arc2D {
        Arc2D {
            center,
            radius,
            start_angle,
            end_angle,
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rect2D {
    pub bottom_left: Point2f,
    pub top_right: Point2f,
}

impl Rect2D {
    pub fn new(bottom_left: Point2f, top_right: Point2f) -> Rect2D {
        Rect2D {
            bottom_left,
            top_right,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Poly2D {
    pub pts: Vec<Point2f>,
}

impl Poly2D {
    pub fn new(pts: Vec<Point2f>) -> Poly2D {
        Poly2D { pts }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Element2D {
    Line(Line2D),
    Arc(Arc2D),
    Rect(Rect2D),
    Poly(Poly2D),
}

///A polygon projected into drawing space, but still in world coordinates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrawElement2D {
    pub element: Element2D,
    pub line_thickness: f32,
    pub line_color: RGBA,
    pub line_type: LineType,
    pub fill_type: FillType,
}

impl DrawElement2D {
    ///Defaults to solid black lines and white solid fill, full opacity
    pub fn new_default(element: Element2D) -> DrawElement2D {
        DrawElement2D {
            element,
            line_thickness: 1.0,
            line_color: RGBA::default(),
            line_type: LineType::Solid,
            fill_type: FillType::default(),
        }
    }
}

///3D triangle mesh data to draw in the model view
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeshData {
    pub positions: Vec<WorldCoord>,
    pub indices: Vec<u64>,
    pub metadata: Option<serde_json::Value>,
}

impl MeshData {
    pub fn push_pt(&mut self, pt: Point3f) {
        //Bake in coordinate transformations to graphical space
        self.positions.push(pt.x);
        self.positions.push(pt.z);
        self.positions.push(-pt.y);
    }
}

///2D line representations of objects to draw in viewports
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrawingData {
    pub elements: Vec<DrawElement2D>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DrawingRepresentations {
    pub top: Option<DrawingData>,
    pub front: Option<DrawingData>,
    pub left: Option<DrawingData>,
    pub right: Option<DrawingData>,
    pub back: Option<DrawingData>,
    pub bottom: Option<DrawingData>,
}

///Used for symbol instances
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstanceData {
    pub transform: TransMat,
    pub bbox: Cube,
    pub source: Option<ObjID>,
    pub metadata: Option<serde_json::Value>,
}

pub fn apply_transform(mat: TransMat, cube: Cube) -> Cube {
    let bottom_left = mat.transform_point(cube.bottom_left);
    let top_right = mat.transform_point(cube.top_right);
    Cube {
        bottom_left,
        top_right,
    }
}

pub fn project_on_line(first: &Point3f, second: &Point3f, project: &Point3f) -> Point3f {
    let dir = second - first;
    let proj_vec = (project - first).project_on(dir);
    first + proj_vec
}

pub fn get_interp_along_line(first: &Point3f, second: &Point3f, project: &Point3f) -> Interp {
    let dir = second - first;
    let proj_vec = (project - first).project_on(dir);
    Interp::new((proj_vec.magnitude2() / dir.magnitude2()).sqrt())
}

pub fn rotate_point_through_angle_2d(origin: &Point3f, point: &Point3f, angle: Radians) -> Point3f {
    let dir = point - origin;
    let rot = cgmath::Matrix3::from_angle_z(angle);
    let rotated = rot * dir;
    origin + rotated
}

pub fn get_perp_2d(first: &Point3f, second: &Point3f) -> Vector3f {
    (second - first).cross(Vector3f::unit_z()).normalize()
}

pub fn graphic_space(pt: &Point3f) -> Point3f {
    Point3f::new(pt.x, pt.z, -pt.y)
}

pub fn minimum_of_list(list: &Vec<f64>) -> Result<f64, ObjError> {
    let mut iter = list.iter();
    let init = iter.next().ok_or(other_error("Need at least one input"))?;
    let result = iter.fold(init, |acc, x| {
        // return None if x is NaN
        let cmp = x.partial_cmp(&acc);
        if let Some(std::cmp::Ordering::Less) = cmp {
            x
        } else {
            acc
        }
    });
    Ok(*result)
}

pub fn maximum_of_list(list: &Vec<f64>) -> Result<f64, ObjError> {
    let mut iter = list.into_iter();
    let init = iter.next().ok_or(other_error("Need at least one input"))?;
    let result = iter.fold(init, |acc, x| {
        // return None if x is NaN
        let cmp = x.partial_cmp(&acc);
        if let Some(std::cmp::Ordering::Greater) = cmp {
            x
        } else {
            acc
        }
    });
    Ok(*result)
}

pub fn average_of_list(list: &Vec<f64>) -> Result<f64, ObjError> {
    let mut iter = list.into_iter();
    let init = *iter.next().ok_or(other_error("Need at least one input"))?;
    let sum = iter.fold(init, |acc, x| acc + x);
    let avg = sum / list.len() as f64;
    Ok(avg)
}

///Returns the union of two bounding boxes
pub fn compose_bboxs(box_1: &Cube, box_2: &Cube) -> Cube {
    let x_vals = vec![
        box_1.bottom_left.x,
        box_1.top_right.x,
        box_2.bottom_left.x,
        box_2.top_right.x,
    ];
    let y_vals = vec![
        box_1.bottom_left.y,
        box_1.top_right.y,
        box_2.bottom_left.y,
        box_2.top_right.y,
    ];
    let z_vals = vec![
        box_1.bottom_left.z,
        box_1.top_right.z,
        box_2.bottom_left.z,
        box_2.top_right.z,
    ];
    let left_x = minimum_of_list(&x_vals).unwrap();
    let left_y = minimum_of_list(&y_vals).unwrap();
    let left_z = minimum_of_list(&z_vals).unwrap();
    let right_x = maximum_of_list(&x_vals).unwrap();
    let right_y = maximum_of_list(&y_vals).unwrap();
    let right_z = maximum_of_list(&z_vals).unwrap();
    let bottom_left = Point3f::new(left_x, left_y, left_z);
    let top_right = Point3f::new(right_x, right_y, right_z);
    Cube {
        bottom_left,
        top_right,
    }
}

pub fn offset_line(
    first_pt: &Point3f,
    second_pt: &Point3f,
    width: WorldCoord,
) -> (Point3f, Point3f, Point3f, Point3f) {
    let dir = second_pt - first_pt;
    let perp = dir.cross(Vector3f::unit_z()).normalize();
    let offset = perp * width;
    let first = first_pt + offset;
    let second = second_pt + offset;
    let third = second_pt - offset;
    let fourth = first_pt - offset;
    (first, second, third, fourth)
}

pub fn get_axis_aligned_bound_box(
    first_pt: &Point3f,
    second_pt: &Point3f,
    width: WorldCoord,
    height: WorldCoord,
) -> Cube {
    let (first, second, third, fourth) = offset_line(first_pt, second_pt, width);
    let vert_offset = Vector3f::new(0.0, 0.0, height);
    let fifth = first + vert_offset;
    let sixth = second + vert_offset;
    let seventh = third + vert_offset;
    let eighth = fourth + vert_offset;
    let x_vals = vec![
        first.x, second.x, third.x, fourth.x, fifth.x, sixth.x, seventh.x, eighth.x,
    ];
    let y_vals = vec![
        first.y, second.y, third.y, fourth.y, fifth.y, sixth.y, seventh.y, eighth.y,
    ];
    let z_vals = vec![
        first.z, second.z, third.z, fourth.z, fifth.z, sixth.z, seventh.z, eighth.z,
    ];
    let bottom_left = Point3f::new(
        minimum_of_list(&x_vals).unwrap(),
        minimum_of_list(&y_vals).unwrap(),
        minimum_of_list(&z_vals).unwrap(),
    );
    let top_right = Point3f::new(
        maximum_of_list(&x_vals).unwrap(),
        maximum_of_list(&y_vals).unwrap(),
        maximum_of_list(&z_vals).unwrap(),
    );
    Cube {
        bottom_left,
        top_right,
    }
}

///A value between 0 and 1
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Interp {
    val: f64,
}

impl Interp {
    pub fn new(mut in_val: f64) -> Interp {
        if in_val > 1.0 {
            in_val = 1.0;
        }
        if in_val < 0.0 {
            in_val = 0.0;
        }
        Interp { val: in_val }
    }

    pub fn val(&self) -> f64 {
        self.val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_on_line() {
        let first = Point3f::new(0.0, 0.0, 0.0);
        let second = Point3f::new(1.0, 0.0, 0.0);
        let project = Point3f::new(0.5, 1.0, 0.0);
        assert_eq!(
            project_on_line(&first, &second, &project),
            Point3f::new(0.5, 0.0, 0.0)
        );

        let first = Point3f::new(0.0, 0.0, 0.0);
        let second = Point3f::new(1.0, 0.0, 0.0);
        let project = Point3f::new(0.5, -1.0, 0.0);
        assert_eq!(
            project_on_line(&first, &second, &project),
            Point3f::new(0.5, 0.0, 0.0)
        );

        let first = Point3f::new(0.0, 0.0, 0.0);
        let second = Point3f::new(1.0, 0.0, 0.0);
        let project = Point3f::new(-1.0, -1.0, 1.0);
        assert_eq!(
            project_on_line(&first, &second, &project),
            Point3f::new(-1.0, 0.0, 0.0)
        );

        let first = Point3f::new(-50.0, 20.0, 0.0);
        let second = Point3f::new(-40.0, 20.0, 0.0);
        let project = Point3f::new(-45.0, 19.0, 0.0);
        assert_eq!(
            project_on_line(&first, &second, &project),
            Point3f::new(-45.0, 20.0, 0.0)
        );
    }

    #[test]
    fn test_get_axis_aligned_bound_box() {
        let pt_1 = Point3f::new(0.0, 0.0, 0.0);
        let pt_2 = Point3f::new(100.0, 0.0, 0.0);
        let width = 1.0;
        let height = 100.0;
        let ref_result = get_axis_aligned_bound_box(&pt_1, &pt_2, width, height);
        assert_eq!(
            ref_result,
            Cube {
                bottom_left: Point3f::new(0.0, -1.0, 0.0),
                top_right: Point3f::new(100.0, 1.0, 100.0)
            }
        );
    }
}
