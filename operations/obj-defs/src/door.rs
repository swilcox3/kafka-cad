use crate::*;
use cgmath::InnerSpace;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Door {
    id: ObjID,
    pub dir: RefLineSeg,
    pub width: WorldCoord,
    pub height: WorldCoord,
}

impl Door {
    pub fn new(first: Point3f, second: Point3f, width: WorldCoord, height: WorldCoord) -> Door {
        let id = ObjID::new_v4();
        Door {
            id: id,
            dir: RefLineSeg::new(Line::new(first, second)),
            width: width,
            height: height,
        }
    }

    fn get_door_points(
        &self,
    ) -> (
        Point3f,
        Point3f,
        Point3f,
        Point3f,
        Point3f,
        Point3f,
        Point3f,
        Point3f,
    ) {
        let (first, second, third, fourth) =
            offset_line(&self.dir.line.pt_1, &self.dir.line.pt_2, self.width);
        let vert_offset = Vector3f::new(0.0, 0.0, self.height);
        let fifth = first + vert_offset;
        let sixth = second + vert_offset;
        let seventh = third + vert_offset;
        let eighth = fourth + vert_offset;
        (first, second, third, fourth, fifth, sixth, seventh, eighth)
    }
}

#[async_trait::async_trait]
#[typetag::serde]
impl Data for Door {
    fn get_id(&self) -> &ObjID {
        &self.id
    }

    fn reset_id(&mut self) {
        self.id = ObjID::new_v4();
    }

    async fn update(&self, conn: &mut dyn GeomKernel) -> Result<UpdateOutput, ObjError> {
        let mut data = MeshData {
            positions: Vec::with_capacity(24),
            indices: Vec::with_capacity(36),
            metadata: Some(json!({
                "type": "Door",
                "traits": ["ReferTo", "Position", "UpdateFromRefs"],
                "obj": {
                    "Width": self.width,
                    "Height": self.height,
                    "First": self.dir.line.pt_1,
                    "Second": self.dir.line.pt_2
                }
            })),
        };
        let rotated = rotate_point_through_angle_2d(
            &self.dir.line.pt_1,
            &self.dir.line.pt_2,
            radians(std::f64::consts::FRAC_PI_4),
        );
        conn.make_prism(
            &self.dir.line.pt_1,
            &rotated,
            self.width,
            self.height,
            &mut data,
        ).await?;
        Ok(UpdateOutput::Mesh { data: data })
    }

    fn get_result(&self, ref_type: RefType, result: ResultInd) -> Option<RefResult> {
        match ref_type {
            RefType::Drawable => Some(RefResult::Empty),
            RefType::Existence => Some(RefResult::Empty),
            RefType::AxisAlignedBoundBox => match result {
                0 => Some(self.get_axis_aligned_bounding_box().as_result()),
                _ => None,
            },
            RefType::ProfilePoint => match result {
                0 => Some(self.dir.line.pt_1.as_result()),
                1 => Some(self.dir.line.pt_2.as_result()),
                _ => None,
            },
            RefType::ProfileLine => match result {
                0 => Some(self.dir.get_result()),
                _ => None,
            },
            RefType::ProfilePlane => match result {
                0 => {
                    let third = Point3f::new(
                        self.dir.line.pt_2.x,
                        self.dir.line.pt_2.y,
                        self.dir.line.pt_2.z + self.height,
                    );
                    Some(Plane::new(self.dir.line.pt_1, self.dir.line.pt_2, third).as_result())
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn get_results_for_type(&self, ref_type: RefType) -> Vec<RefResult> {
        match ref_type {
            RefType::Drawable => vec![RefResult::Empty],
            RefType::Existence => vec![RefResult::Empty],
            RefType::AxisAlignedBoundBox => vec![self.get_axis_aligned_bounding_box().as_result()],
            RefType::ProfilePoint => vec![
                self.dir.line.pt_1.as_result(),
                self.dir.line.pt_2.as_result(),
            ],
            RefType::ProfileLine => vec![self.dir.get_result()],
            RefType::ProfilePlane => {
                let third = Point3f::new(
                    self.dir.line.pt_2.x,
                    self.dir.line.pt_2.y,
                    self.dir.line.pt_2.z + self.height,
                );
                vec![Plane::new(self.dir.line.pt_1, self.dir.line.pt_2, third).as_result()]
            }
            _ => Vec::new(),
        }
    }

    fn get_num_results_for_type(&self, ref_type: RefType) -> usize {
        match ref_type {
            RefType::Drawable => 1,
            RefType::Existence => 1,
            RefType::AxisAlignedBoundBox => 1,
            RefType::ProfilePoint => 2,
            RefType::ProfileLine => 1,
            RefType::ProfilePlane => 1,
            _ => 0,
        }
    }

    fn clear_refs(&mut self) {
        self.dir.refer = None;
    }

    fn get_refs(&self) -> Vec<Option<Reference>> {
        let mut results = Vec::new();
        let self_id_point_0 = RefID::new(self.id, RefType::ProfilePoint, 0);
        let self_id_point_1 = RefID::new(self.id, RefType::ProfilePoint, 1);
        let self_id_line = RefID::new(self.id, RefType::ProfileLine, 0);
        let self_id_plane = RefID::new(self.id, RefType::ProfilePlane, 0);
        let self_id_bbox = RefID::new(self.id, RefType::AxisAlignedBoundBox, 0);
        if let Some(id) = &self.dir.refer {
            results.push(Some(Reference::new(self_id_line, id.clone())));
        } else {
            results.push(None);
        }
        results.push(Some(Reference {
            owner: self_id_bbox,
            other: self_id_line,
        }));
        results.push(Some(Reference {
            owner: self_id_point_0,
            other: self_id_line,
        }));
        results.push(Some(Reference {
            owner: self_id_point_1,
            other: self_id_line,
        }));
        results.push(Some(Reference {
            owner: self_id_plane,
            other: self_id_line,
        }));
        results
    }

    fn get_available_refs_for_type(&self, ref_type: RefType) -> Vec<ResultInd> {
        let mut results = Vec::new();
        if let RefType::ProfileLine = ref_type {
            if let None = self.dir.refer {
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
        snap_pt: &Option<RefResult>,
    ) {
        if let RefType::ProfileLine = ref_type {
            match index {
                0 => self.dir.set_reference(result, other_ref, snap_pt),
                _ => (),
            }
        }
    }

    fn add_ref(&mut self, _: RefType, _: RefResult, _: RefID, _: &Option<RefResult>) -> bool {
        return false;
    }

    fn delete_ref(&mut self, ref_type: RefType, index: ResultInd) {
        if let RefType::ProfileLine = ref_type {
            match index {
                0 => self.dir.refer = None,
                _ => (),
            }
        }
    }

    fn set_associated_result_for_type(
        &mut self,
        ref_type: RefType,
        index: ResultInd,
        result_opt: Option<RefResult>,
    ) {
        if let RefType::ProfileLine = ref_type {
            match index {
                0 => {
                    if let Some(result) = result_opt {
                        self.dir.update(result, &None);
                    }
                }
                _ => (),
            }
        }
    }

    fn data_clone(&self) -> DataBox {
        Box::new(self.clone())
    }

    fn as_position(&self) -> Option<&dyn Position> {
        Some(self)
    }

    fn as_position_mut(&mut self) -> Option<&mut dyn Position> {
        Some(self)
    }

    fn as_drawing_views(&self) -> Option<&dyn DrawingViews> {
        Some(self)
    }
}

impl Position for Door {
    fn move_obj(&mut self, delta: &Vector3f) {
        self.dir.line.pt_1 += *delta;
        self.dir.line.pt_2 += *delta;
    }

    fn get_axis_aligned_bounding_box(&self) -> Cube {
        get_axis_aligned_bound_box(
            &self.dir.line.pt_1,
            &self.dir.line.pt_2,
            self.width,
            self.height,
        )
    }
}

impl DrawingViews for Door {
    fn get_top(&self) -> DrawingData {
        let line_1 = Line2D::new(x_y(&self.dir.line.pt_1), x_y(&self.dir.line.pt_2));
        let rotated = rotate_point_through_angle_2d(
            &self.dir.line.pt_1,
            &self.dir.line.pt_2,
            radians(std::f64::consts::FRAC_PI_2),
        );
        let line_2 = Line2D::new(x_y(&self.dir.line.pt_1), x_y(&rotated));
        let length = (line_1.first - line_1.second).magnitude();
        let arc = Arc2D::new(
            x_y(&self.dir.line.pt_1),
            length,
            radians(0.0),
            radians(std::f64::consts::FRAC_PI_2),
        );
        let elements = vec![
            DrawElement2D::new_default(Element2D::Line(line_1)),
            DrawElement2D::new_default(Element2D::Line(line_2)),
            DrawElement2D::new_default(Element2D::Arc(arc)),
        ];
        DrawingData { elements }
    }

    fn get_front(&self) -> DrawingData {
        let (first, second, third, fourth, fifth, sixth, seventh, eighth) = self.get_door_points();
        let rect_1 = Rect2D::new(x_z(&first), x_z(&sixth));
        let rect_2 = Rect2D::new(x_z(&fourth), x_z(&fifth));
        let rect_3 = Rect2D::new(x_z(&second), x_z(&seventh));
        let rect_4 = Rect2D::new(x_z(&third), x_z(&eighth));
        let rect_1_y = average_of_list(&vec![first.y, sixth.y]).unwrap();
        let rect_2_y = average_of_list(&vec![fourth.y, fifth.y]).unwrap();
        let rect_3_y = average_of_list(&vec![second.y, seventh.y]).unwrap();
        let rect_4_y = average_of_list(&vec![third.y, eighth.y]).unwrap();
        let mut rects = vec![
            (rect_1_y, rect_1),
            (rect_2_y, rect_2),
            (rect_3_y, rect_3),
            (rect_4_y, rect_4),
        ];
        //Sort by y from greatest to least
        rects.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let results = rects
            .drain(0..)
            .map(|(_, rect)| DrawElement2D::new_default(Element2D::Rect(rect)))
            .collect();
        DrawingData { elements: results }
    }

    fn get_left(&self) -> DrawingData {
        let (first, second, third, fourth, fifth, sixth, seventh, eighth) = self.get_door_points();
        let rect_1 = Rect2D::new(y_z(&first), y_z(&sixth));
        let rect_2 = Rect2D::new(y_z(&fourth), y_z(&fifth));
        let rect_3 = Rect2D::new(y_z(&second), y_z(&seventh));
        let rect_4 = Rect2D::new(y_z(&third), y_z(&eighth));
        let rect_1_x = average_of_list(&vec![first.x, sixth.x]).unwrap();
        let rect_2_x = average_of_list(&vec![fourth.x, fifth.x]).unwrap();
        let rect_3_x = average_of_list(&vec![second.x, seventh.x]).unwrap();
        let rect_4_x = average_of_list(&vec![third.x, eighth.x]).unwrap();
        let mut rects = vec![
            (rect_1_x, rect_1),
            (rect_2_x, rect_2),
            (rect_3_x, rect_3),
            (rect_4_x, rect_4),
        ];
        //Sort by x from greatest to least
        rects.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
        let results = rects
            .drain(0..)
            .map(|(_, rect)| DrawElement2D::new_default(Element2D::Rect(rect)))
            .collect();
        DrawingData { elements: results }
    }

    fn get_right(&self) -> DrawingData {
        let (first, second, third, fourth, fifth, sixth, seventh, eighth) = self.get_door_points();
        let rect_1 = Rect2D::new(y_z(&first), y_z(&sixth));
        let rect_2 = Rect2D::new(y_z(&fourth), y_z(&fifth));
        let rect_3 = Rect2D::new(y_z(&second), y_z(&seventh));
        let rect_4 = Rect2D::new(y_z(&third), y_z(&eighth));
        let rect_1_x = average_of_list(&vec![first.x, sixth.x]).unwrap();
        let rect_2_x = average_of_list(&vec![fourth.x, fifth.x]).unwrap();
        let rect_3_x = average_of_list(&vec![second.x, seventh.x]).unwrap();
        let rect_4_x = average_of_list(&vec![third.x, eighth.x]).unwrap();
        let mut rects = vec![
            (rect_1_x, rect_1),
            (rect_2_x, rect_2),
            (rect_3_x, rect_3),
            (rect_4_x, rect_4),
        ];
        //Sort by x from greatest to least
        rects.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let results = rects
            .drain(0..)
            .map(|(_, rect)| DrawElement2D::new_default(Element2D::Rect(rect)))
            .collect();
        DrawingData { elements: results }
    }

    fn get_back(&self) -> DrawingData {
        let (first, second, third, fourth, fifth, sixth, seventh, eighth) = self.get_door_points();
        let rect_1 = Rect2D::new(x_z(&first), x_z(&sixth));
        let rect_2 = Rect2D::new(x_z(&fourth), x_z(&fifth));
        let rect_3 = Rect2D::new(x_z(&second), x_z(&seventh));
        let rect_4 = Rect2D::new(x_z(&third), x_z(&eighth));
        let rect_1_y = average_of_list(&vec![first.y, sixth.y]).unwrap();
        let rect_2_y = average_of_list(&vec![fourth.y, fifth.y]).unwrap();
        let rect_3_y = average_of_list(&vec![second.y, seventh.y]).unwrap();
        let rect_4_y = average_of_list(&vec![third.y, eighth.y]).unwrap();
        let mut rects = vec![
            (rect_1_y, rect_1),
            (rect_2_y, rect_2),
            (rect_3_y, rect_3),
            (rect_4_y, rect_4),
        ];
        //Sort by y from greatest to least
        rects.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let results = rects
            .drain(0..)
            .map(|(_, rect)| DrawElement2D::new_default(Element2D::Rect(rect)))
            .collect();
        DrawingData { elements: results }
    }

    fn get_bottom(&self) -> DrawingData {
        let line_1 = Line2D::new(x_y(&self.dir.line.pt_1), x_y(&self.dir.line.pt_2));
        let rotated = rotate_point_through_angle_2d(
            &self.dir.line.pt_1,
            &self.dir.line.pt_2,
            radians(std::f64::consts::FRAC_PI_2),
        );
        let line_2 = Line2D::new(x_y(&self.dir.line.pt_1), x_y(&rotated));
        let length = (line_1.first - line_1.second).magnitude();
        let arc = Arc2D::new(
            x_y(&self.dir.line.pt_1),
            length,
            radians(0.0),
            radians(std::f64::consts::FRAC_PI_2),
        );
        let elements = vec![
            DrawElement2D::new_default(Element2D::Line(line_1)),
            DrawElement2D::new_default(Element2D::Line(line_2)),
            DrawElement2D::new_default(Element2D::Arc(arc)),
        ];
        DrawingData { elements }
    }
}
