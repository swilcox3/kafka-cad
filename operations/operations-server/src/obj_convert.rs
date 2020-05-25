use crate::*;
use operations::indexmap::IndexMap;
use representation::*;

pub fn to_obj_id(id: &str) -> Result<ObjID, tonic::Status> {
    let parsed =
        ObjID::parse_str(id).map_err(|e| tonic::Status::invalid_argument(format!("{:?}", e)))?;
    Ok(parsed)
}

pub fn to_point_2u(msg: &Option<Point2Msg>) -> Result<Point2f, tonic::Status> {
    if let Some(pt_msg) = msg {
        Ok(Point2f::new(pt_msg.x, pt_msg.y))
    } else {
        Err(tonic::Status::invalid_argument("No point passed in"))
    }
}

pub fn to_point_3f(msg: &Option<Point3Msg>) -> Result<Point3f, tonic::Status> {
    if let Some(pt_msg) = msg {
        Ok(Point3f::new(pt_msg.x, pt_msg.y, pt_msg.z))
    } else {
        Err(tonic::Status::invalid_argument("No point passed in"))
    }
}

pub fn to_door(
    first_pt: &Option<Point3Msg>,
    second_pt: &Option<Point3Msg>,
    width: WorldCoord,
    height: WorldCoord,
) -> Result<Door, tonic::Status> {
    Ok(Door::new(
        to_point_3f(first_pt)?,
        to_point_3f(second_pt)?,
        width,
        height,
    ))
}

pub fn to_wall(
    first_pt: &Option<Point3Msg>,
    second_pt: &Option<Point3Msg>,
    width: WorldCoord,
    height: WorldCoord,
) -> Result<Wall, tonic::Status> {
    Ok(Wall::new(
        to_point_3f(first_pt)?,
        to_point_3f(second_pt)?,
        width,
        height,
    ))
}

pub fn get_view_type(view_msg: &str) -> Result<ViewType, tonic::Status> {
    match serde_json::from_str(view_msg) {
        Ok(view) => Ok(view),
        Err(e) => Err(tonic::Status::invalid_argument(format!(
            "Invalid json for view type: {:?}",
            e
        ))),
    }
}

pub fn from_object_msg(msg: &ObjectMsg) -> Result<DataBox, ObjError> {
    let obj: DataBox = bincode::deserialize(&msg.obj_data)?;
    Ok(obj)
}

pub fn from_ref_msgs(msgs: &Vec<ReferenceMsg>) -> Result<Vec<Reference>, tonic::Status> {
    let mut results = Vec::new();
    for msg in msgs {
        results.push(Reference {
            owner: from_ref_id_msg(&msg.owner)?,
            other: from_ref_id_msg(&msg.other)?,
        });
    }
    Ok(results)
}

fn from_ref_id_msg(msg: &Option<RefIdMsg>) -> Result<RefID, tonic::Status> {
    match msg {
        Some(msg) => Ok(RefID {
            id: to_obj_id(&msg.id)?,
            ref_type: match ref_id_msg::RefType::from_i32(msg.ref_type) {
                Some(ref_id_msg::RefType::Drawable) => RefType::Drawable,
                Some(ref_id_msg::RefType::Existence) => RefType::Existence,
                Some(ref_id_msg::RefType::AxisAlignedBbox) => RefType::AxisAlignedBoundBox,
                Some(ref_id_msg::RefType::ProfilePoint) => RefType::ProfilePoint,
                Some(ref_id_msg::RefType::ProfileLine) => RefType::ProfileLine,
                Some(ref_id_msg::RefType::ProfilePlane) => RefType::ProfilePlane,
                Some(ref_id_msg::RefType::Property) => RefType::Property,
                Some(ref_id_msg::RefType::Empty) => RefType::Empty,
                None => return Err(tonic::Status::invalid_argument("No ref type set")),
            },
            index: msg.index as ResultInd,
        }),
        None => Err(tonic::Status::invalid_argument("No ref id passed in")),
    }
}

fn to_ref_id_msg(ref_id: &RefID) -> RefIdMsg {
    RefIdMsg {
        id: ref_id.id.to_string(),
        ref_type: match ref_id.ref_type {
            RefType::Drawable => ref_id_msg::RefType::Drawable as i32,
            RefType::Existence => ref_id_msg::RefType::Existence as i32,
            RefType::AxisAlignedBoundBox => ref_id_msg::RefType::AxisAlignedBbox as i32,
            RefType::ProfilePoint => ref_id_msg::RefType::ProfilePoint as i32,
            RefType::ProfileLine => ref_id_msg::RefType::ProfileLine as i32,
            RefType::ProfilePlane => ref_id_msg::RefType::ProfilePlane as i32,
            RefType::Property => ref_id_msg::RefType::Property as i32,
            RefType::Empty => ref_id_msg::RefType::Empty as i32,
        },
        index: ref_id.index as u64,
    }
}

pub fn to_object_msg(obj: &DataBox) -> Result<ObjectMsg, ObjError> {
    let refs = obj.get_refs();
    let mut ref_msgs = Vec::new();
    for refer_opt in refs {
        match refer_opt {
            Some(refer) => {
                ref_msgs.push(OptionReferenceMsg {
                    reference: Some(ReferenceMsg {
                        owner: Some(to_ref_id_msg(&refer.owner)),
                        other: Some(to_ref_id_msg(&refer.other)),
                    }),
                });
            }
            None => {
                ref_msgs.push(OptionReferenceMsg { reference: None });
            }
        }
    }
    Ok(ObjectMsg {
        id: obj.get_id().to_string(),
        dependencies: Some(DependenciesMsg {
            references: ref_msgs,
        }),
        obj_data: bincode::serialize(obj)?,
    })
}

pub fn get_map_from_change_msgs(
    msgs: &Vec<ChangeMsg>,
) -> Result<IndexMap<ObjID, Option<DataBox>>, tonic::Status> {
    let mut results = IndexMap::new();
    for msg in msgs {
        match &msg.change_type {
            Some(change_msg::ChangeType::Add(object))
            | Some(change_msg::ChangeType::Modify(object)) => {
                let id = to_obj_id(&object.id)?;
                results.insert(id, Some(from_object_msg(&object).map_err(to_status)?));
            }
            Some(change_msg::ChangeType::Delete(msg)) => {
                let id = to_obj_id(&msg.id)?;
                results.insert(id, None);
            }
            None => (),
        }
    }
    Ok(results)
}

pub fn from_change_msg(msg: &ChangeMsg) -> Result<Change, tonic::Status> {
    match &msg.change_type {
        Some(change_msg::ChangeType::Add(object)) => Ok(Change::Add {
            obj: from_object_msg(&object).map_err(to_status)?,
        }),
        Some(change_msg::ChangeType::Modify(object)) => Ok(Change::Modify {
            obj: from_object_msg(&object).map_err(to_status)?,
        }),
        Some(change_msg::ChangeType::Delete(msg)) => Ok(Change::Delete {
            id: to_obj_id(&msg.id)?,
        }),
        None => Err(tonic::Status::invalid_argument("No change type specified")),
    }
}

pub fn from_change_msgs(msgs: &Vec<ChangeMsg>) -> Result<Vec<Change>, tonic::Status> {
    let mut results = Vec::new();
    for msg in msgs {
        results.push(from_change_msg(msg)?);
    }
    Ok(results)
}

pub fn to_change_msgs(
    old_changes: &Vec<ChangeMsg>,
    objects: &IndexMap<ObjID, Option<DataBox>>,
) -> Result<Vec<ChangeMsg>, ObjError> {
    let mut results = Vec::new();
    for i in 0..old_changes.len() {
        if let Some(old_change) = old_changes.get(i) {
            if let Some((id, obj_opt)) = objects.get_index(i) {
                match old_change.change_type {
                    Some(change_msg::ChangeType::Add(..)) => {
                        if let Some(obj) = obj_opt {
                            let change = ChangeMsg {
                                user: old_change.user.clone(),
                                change_type: Some(change_msg::ChangeType::Add(to_object_msg(obj)?)),
                                change_source: old_change.change_source.clone(),
                            };
                            results.push(change);
                        }
                    }
                    Some(change_msg::ChangeType::Modify(..)) => {
                        if let Some(obj) = obj_opt {
                            let change = ChangeMsg {
                                user: old_change.user.clone(),
                                change_type: Some(change_msg::ChangeType::Modify(to_object_msg(
                                    obj,
                                )?)),
                                change_source: old_change.change_source.clone(),
                            };
                            results.push(change);
                        }
                    }
                    Some(change_msg::ChangeType::Delete(..)) => {
                        let change = ChangeMsg {
                            user: old_change.user.clone(),
                            change_type: Some(change_msg::ChangeType::Delete(DeleteMsg {
                                id: id.to_string(),
                            })),
                            change_source: old_change.change_source.clone(),
                        };
                        results.push(change);
                    }
                    None => {
                        results.push(ChangeMsg {
                            user: old_change.user.clone(),
                            change_type: None,
                            change_source: old_change.change_source.clone(),
                        });
                    }
                }
            }
        }
    }
    Ok(results)
}

fn from_json(json_opt: Option<serde_json::Value>) -> String {
    match json_opt {
        Some(json) => json.to_string(),
        None => String::default(),
    }
}

fn encode_mesh(mesh: MeshData) -> MeshDataMsg {
    MeshDataMsg {
        positions: mesh.positions,
        indices: mesh.indices,
        meta_json: from_json(mesh.metadata),
    }
}

fn encode_transmat(mat: TransMat) -> Vec<f64> {
    let mut results = Vec::new();
    results.push(mat.x.x);
    results.push(mat.x.y);
    results.push(mat.x.z);
    results.push(mat.x.w);
    results.push(mat.y.x);
    results.push(mat.y.y);
    results.push(mat.y.z);
    results.push(mat.y.w);
    results.push(mat.z.x);
    results.push(mat.z.y);
    results.push(mat.z.z);
    results.push(mat.z.w);
    results.push(mat.w.x);
    results.push(mat.w.y);
    results.push(mat.w.z);
    results.push(mat.w.w);
    results
}

fn encode_point3(pt: Point3f) -> Option<Point3Msg> {
    Some(Point3Msg {
        x: pt.x,
        y: pt.y,
        z: pt.z,
    })
}

fn encode_point2(pt: Point2f) -> Option<Point2Msg> {
    Some(Point2Msg { x: pt.x, y: pt.y })
}

fn encode_instance(instance: InstanceData) -> InstanceDataMsg {
    let source = match instance.source {
        Some(id) => id.to_string(),
        None => String::new(),
    };
    let meta_json = match instance.metadata {
        Some(meta) => meta.to_string(),
        None => String::new(),
    };
    InstanceDataMsg {
        transform: encode_transmat(instance.transform),
        bottom_left: encode_point3(instance.bbox.bottom_left),
        top_right: encode_point3(instance.bbox.top_right),
        source,
        meta_json,
    }
}

fn encode_rgba(color: RGBA) -> Option<RgbaMsg> {
    Some(RgbaMsg {
        r: color.r as u32,
        g: color.g as u32,
        b: color.b as u32,
        a: color.a,
    })
}

fn encode_draw_element_2d(element: DrawElement2D) -> DrawElement2DMsg {
    let element_msg = match element.element {
        Element2D::Line(line) => draw_element2_d_msg::Element::Line(Line2DMsg {
            first: encode_point2(line.first),
            second: encode_point2(line.second),
        }),
        Element2D::Arc(arc) => draw_element2_d_msg::Element::Arc(Arc2DMsg {
            center: encode_point2(arc.center),
            radius: arc.radius,
            start_angle: arc.start_angle.0,
            end_angle: arc.end_angle.0,
        }),
        Element2D::Rect(rect) => draw_element2_d_msg::Element::Rect(Rect2DMsg {
            bottom_left: encode_point2(rect.bottom_left),
            top_right: encode_point2(rect.top_right),
        }),
        Element2D::Poly(poly) => {
            let mut pts = Vec::new();
            for pt in poly.pts {
                pts.push(encode_point2(pt).unwrap());
            }
            draw_element2_d_msg::Element::Poly(Poly2DMsg { pts })
        }
    };
    let fill_type = match element.fill_type {
        FillType::Solid { color } => {
            draw_element2_d_msg::FillType::FillSolid(encode_rgba(color).unwrap())
        }
        FillType::Hatch { name } => draw_element2_d_msg::FillType::Hatch(name),
    };
    let line_type = match element.line_type {
        LineType::Solid => draw_element2_d_msg::LineType::LineSolid(String::default()),
        LineType::Dashed { name } => draw_element2_d_msg::LineType::Dashed(name),
    };
    DrawElement2DMsg {
        element: Some(element_msg),
        line_type: Some(line_type),
        fill_type: Some(fill_type),
        line_thickness: element.line_thickness,
        line_color: encode_rgba(element.line_color),
    }
}

fn encode_drawing_data(data_opt: Option<DrawingData>) -> Option<DrawingDataMsg> {
    match data_opt {
        Some(data) => {
            let mut elements = Vec::new();
            for elem in data.elements {
                elements.push(encode_draw_element_2d(elem));
            }
            Some(DrawingDataMsg { elements })
        }
        None => None,
    }
}

pub fn encode_views(views_opt: Option<DrawingRepresentations>) -> Option<DrawingViewsMsg> {
    match views_opt {
        Some(views) => Some(DrawingViewsMsg {
            top: encode_drawing_data(views.top),
            front: encode_drawing_data(views.front),
            left: encode_drawing_data(views.left),
            right: encode_drawing_data(views.right),
            back: encode_drawing_data(views.back),
            bottom: encode_drawing_data(views.bottom),
        }),
        None => None,
    }
}

pub fn encode_update_output(
    output: UpdateOutput,
    views: Option<DrawingRepresentations>,
) -> UpdateOutputMsg {
    let encoded_output = match output {
        UpdateOutput::Empty => Some(update_output_msg::Output::Empty(String::default())),
        UpdateOutput::Delete => Some(update_output_msg::Output::Delete(String::default())),
        UpdateOutput::Mesh { data } => Some(update_output_msg::Output::Mesh(encode_mesh(data))),
        UpdateOutput::FileRef { file } => {
            Some(update_output_msg::Output::FileRef(file.to_string()))
        }
        UpdateOutput::Instance { data } => {
            Some(update_output_msg::Output::Instance(encode_instance(data)))
        }
        UpdateOutput::Other { data } => {
            Some(update_output_msg::Output::OtherJson(data.to_string()))
        }
    };
    let encoded_views = encode_views(views);
    UpdateOutputMsg {
        output: encoded_output,
        views: encoded_views,
    }
}
