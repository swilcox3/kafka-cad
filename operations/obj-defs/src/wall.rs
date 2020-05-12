use crate::*;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Wall {
    pub first_pt: UpdatableInfo<Point3f>,
    pub second_pt: UpdatableInfo<Point3f>,
    pub width: WorldCoord,
    pub height: WorldCoord,
    openings: Vec<Option<UpdatableInfo<Plane>>>,
    id: ObjID,
}

impl Wall {
    pub fn new(first: Point3f, second: Point3f, width: WorldCoord, height: WorldCoord) -> Wall {
        let id = ObjID::new_v4();
        Wall {
            id,
            first_pt: UpdatableInfo::new(first),
            second_pt: UpdatableInfo::new(second),
            width: width,
            height: height,
            openings: Vec::new(),
        }
    }

    fn get_wall_points(
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
            offset_line(&self.first_pt.info, &self.second_pt.info, self.width);
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
impl Data for Wall {
    fn get_id(&self) -> &ObjID {
        &self.id
    }

    fn reset_id(&mut self) {
        self.id = ObjID::new_v4();
    }

    async fn update(&self, geom_conn: &mut dyn GeomKernel) -> Result<UpdateOutput, ObjError> {
        let mut data = MeshData {
            positions: Vec::new(),
            indices: Vec::new(),
            metadata: Some(json! ({
                "type": "Wall",
                "traits": ["Position"],
                "obj": {
                    "Width": self.width,
                    "Height": self.height,
                    "First": self.first_pt.info,
                    "Second": self.second_pt.info
                }
            })),
        };
        geom_conn
            .make_prism(
                &self.first_pt.info,
                &self.second_pt.info,
                self.width,
                self.height,
                &mut data,
            )
            .await?;
        Ok(UpdateOutput::Mesh { data })
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
                0 => Some(self.first_pt.get_result()),
                1 => Some(self.second_pt.get_result()),
                _ => None,
            },
            RefType::ProfileLine => match result {
                0 => Some(Line::new(self.first_pt.info, self.second_pt.info).as_result()),
                _ => None,
            },
            RefType::ProfilePlane => match result {
                _ => {
                    if let Some(open_opt) = self.openings.get(result) {
                        if let Some(open) = open_opt {
                            Some(open.get_result())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
            },
            _ => None,
        }
    }

    fn get_results_for_type(&self, ref_type: RefType) -> Vec<RefResult> {
        match ref_type {
            RefType::Drawable => vec![RefResult::Empty],
            RefType::Existence => vec![RefResult::Empty],
            RefType::AxisAlignedBoundBox => vec![self.get_axis_aligned_bounding_box().as_result()],
            RefType::ProfilePoint => vec![self.first_pt.get_result(), self.second_pt.get_result()],
            RefType::ProfileLine => {
                vec![Line::new(self.first_pt.info, self.second_pt.info).as_result()]
            }
            RefType::ProfilePlane => {
                let mut results = Vec::new();
                for open_opt in &self.openings {
                    if let Some(open) = open_opt {
                        results.push(open.get_result())
                    }
                }
                results
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
            RefType::ProfilePlane => self.openings.len(),
            _ => 0,
        }
    }

    fn clear_refs(&mut self) {
        self.first_pt.refer = None;
        self.second_pt.refer = None;
        for open_opt in &mut self.openings {
            if let Some(open) = open_opt {
                open.refer = None;
            }
        }
    }

    fn get_refs(&self) -> Vec<Option<Reference>> {
        let mut results = Vec::new();
        let self_pt_0 = RefID::new(self.id, RefType::ProfilePoint, 0);
        let self_pt_1 = RefID::new(self.id, RefType::ProfilePoint, 1);
        let self_line = RefID::new(self.id, RefType::ProfileLine, 0);
        let self_bbox = RefID::new(self.id, RefType::AxisAlignedBoundBox, 0);
        if let Some(id) = &self.first_pt.refer {
            results.push(Some(Reference::new(self_pt_0, *id)));
        } else {
            results.push(None);
        }
        if let Some(id) = &self.second_pt.refer {
            results.push(Some(Reference::new(self_pt_1, *id)));
        } else {
            results.push(None);
        }
        results.push(Some(Reference {
            owner: self_bbox,
            other: self_pt_0,
        }));
        results.push(Some(Reference {
            owner: self_bbox,
            other: self_pt_1,
        }));
        results.push(Some(Reference {
            owner: self_line,
            other: self_pt_0,
        }));
        results.push(Some(Reference {
            owner: self_line,
            other: self_pt_1,
        }));
        let mut index = 0;
        for open_opt in &self.openings {
            if let Some(open) = open_opt {
                if let Some(id) = &open.refer {
                    let ref_id = RefID::new(self.id, RefType::ProfilePlane, index);
                    results.push(Some(Reference::new(ref_id, *id)));
                } else {
                    results.push(None);
                }
            } else {
                results.push(None);
            }
            index += 1;
        }
        results
    }

    fn get_available_refs_for_type(&self, ref_type: RefType) -> Vec<ResultInd> {
        let mut results = Vec::new();
        match ref_type {
            RefType::ProfilePoint => {
                if let None = self.first_pt.refer {
                    results.push(0);
                }
                if let None = self.second_pt.refer {
                    results.push(1);
                }
            }
            RefType::ProfilePlane => {
                let mut index = 0;
                for open_opt in &self.openings {
                    if let Some(open) = open_opt {
                        if let None = open.refer {
                            results.push(index);
                        }
                    } else {
                        results.push(index);
                    }
                    index += 1;
                }
            }
            _ => (),
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
        match ref_type {
            RefType::ProfilePoint => match index {
                0 => self.first_pt.set_reference(result, other_ref),
                1 => self.second_pt.set_reference(result, other_ref),
                _ => (),
            },
            RefType::ProfilePlane => {
                if let Some(open_opt) = self.openings.get_mut(index) {
                    if let Some(open) = open_opt {
                        open.set_reference(result, other_ref);
                    } else {
                        if let RefResult::Plane(plane) = result {
                            let mut new_open =
                                UpdatableInfo::new(Plane::new(plane.pt_1, plane.pt_2, plane.pt_3));
                            new_open.set_reference(result, other_ref);
                            *open_opt = Some(new_open);
                        }
                    }
                }
            }
            _ => (),
        }
    }

    fn add_ref(
        &mut self,
        ref_type: RefType,
        result: RefResult,
        other_ref: RefID,
        _extra: &Option<RefResult>,
    ) -> bool {
        if let RefType::ProfilePlane = ref_type {
            if let RefResult::Plane(plane) = result {
                let mut new_open =
                    UpdatableInfo::new(Plane::new(plane.pt_1, plane.pt_2, plane.pt_3));
                new_open.set_reference(result, other_ref);
                self.openings.push(Some(new_open));
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn delete_ref(&mut self, ref_type: RefType, index: ResultInd) {
        match ref_type {
            RefType::ProfilePoint => match index {
                0 => self.first_pt.refer = None,
                1 => self.second_pt.refer = None,
                _ => (),
            },
            RefType::ProfilePlane => {
                if let Some(open_opt) = self.openings.get_mut(index) {
                    *open_opt = None;
                }
            }
            _ => (),
        }
    }

    fn set_associated_result_for_type(
        &mut self,
        ref_type: RefType,
        index: ResultInd,
        result: Option<RefResult>,
    ) {
        match ref_type {
            RefType::ProfilePoint => match index {
                0 => self.first_pt.update(result),
                1 => self.second_pt.update(result),
                _ => (),
            },
            RefType::ProfilePlane => match index {
                _ => {
                    if let Some(open_opt) = self.openings.get_mut(index) {
                        if let Some(open) = open_opt {
                            open.update(result);
                        }
                    }
                }
            },
            _ => (),
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

impl Position for Wall {
    fn move_obj(&mut self, delta: &Vector3f) {
        self.first_pt.info += *delta;
        self.second_pt.info += *delta;
    }

    fn get_axis_aligned_bounding_box(&self) -> Cube {
        get_axis_aligned_bound_box(
            &self.first_pt.info,
            &self.second_pt.info,
            self.width,
            self.height,
        )
    }
}

impl DrawingViews for Wall {
    fn get_top(&self) -> DrawingData {
        let (first, _second, third, _fourth) =
            offset_line(&self.first_pt.info, &self.second_pt.info, self.width);
        let rect = Rect2D::new(x_y(&first), x_y(&third));
        DrawingData {
            elements: vec![DrawElement2D::new_default(Element2D::Rect(rect))],
        }
    }
    fn get_front(&self) -> DrawingData {
        let (first, second, third, fourth, fifth, sixth, seventh, eighth) = self.get_wall_points();
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
        let (first, second, third, fourth, fifth, sixth, seventh, eighth) = self.get_wall_points();
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
        let (first, second, third, fourth, fifth, sixth, seventh, eighth) = self.get_wall_points();
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
        let (first, second, third, fourth, fifth, sixth, seventh, eighth) = self.get_wall_points();
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
        let (first, _second, third, _fourth) =
            offset_line(&self.first_pt.info, &self.second_pt.info, self.width);
        let rect = Rect2D::new(x_y(&first), x_y(&third));
        DrawingData {
            elements: vec![DrawElement2D::new_default(Element2D::Rect(rect))],
        }
    }
}
