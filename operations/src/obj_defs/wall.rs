use crate::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct WallProps {
    #[serde(rename = "Width")]
    pub width: WorldCoord,
    #[serde(rename = "Height")]
    pub height: WorldCoord,
}

#[derive(Debug)]
pub struct Wall {
    id: String,
    pub first_pt: Point3f,
    pub second_pt: Point3f,
    pub props: WallProps,
}

impl Wall {
    pub fn new(wall: WallMsg) -> Result<Wall, OpsError> {
        let id = id_gen::gen_id();
        Ok(Wall {
            id,
            first_pt: to_pt3f(&wall.first_pt)?,
            second_pt: to_pt3f(&wall.second_pt)?,
            props: WallProps {
                width: wall.width,
                height: wall.height,
            },
        })
    }
}

#[tonic::async_trait]
impl Data for Wall {
    fn from_object_msg(id: String, msg: &ObjectMsg) -> Result<Self, OpsError> {
        Ok(Wall {
            id,
            first_pt: into_pt3f(msg.get_profile_pt(0))?,
            second_pt: into_pt3f(msg.get_profile_pt(1))?,
            props: props(msg)?
        })
    }

    fn to_object_msg(self) -> ObjectMsg {
        ObjectMsg {
            dependencies: Some(DependenciesMsg {
                references: vec![
                    OptionReferenceMsg {
                        reference: Some(ReferenceMsg {
                            owner: Some(RefIdMsg {
                                id: self.id.clone(),
                                ref_type: ref_id_msg::RefType::ProfileLine as i32,
                                index: 0,
                            }),
                            other: Some(RefIdMsg {
                                id: self.id.clone(),
                                ref_type: ref_id_msg::RefType::ProfilePoint as i32,
                                index: 0,
                            }),
                        }),
                    },
                    OptionReferenceMsg {
                        reference: Some(ReferenceMsg {
                            owner: Some(RefIdMsg {
                                id: self.id.clone(),
                                ref_type: ref_id_msg::RefType::ProfileLine as i32,
                                index: 0,
                            }),
                            other: Some(RefIdMsg {
                                id: self.id.clone(),
                                ref_type: ref_id_msg::RefType::ProfilePoint as i32,
                                index: 1,
                            }),
                        }),
                    },
                ],
            }),
            results: Some(ResultsMsg {
                visible: true,
                profile: Some(ProfileMsg {
                    points: vec![
                        OptionPoint3Msg::new(to_pt_msg(&self.first_pt)),
                        OptionPoint3Msg::new(to_pt_msg(&self.second_pt)),
                    ],
                    lines: vec![OptionLineMsg {
                        line: Some(LineMsg {
                            first: to_pt_msg(&self.first_pt),
                            second: to_pt_msg(&self.second_pt),
                        }),
                    }],
                    planes: Vec::new(),
                }),
                bbox: bbox(
                    &self.first_pt,
                    &self.second_pt,
                    self.props.width,
                    self.props.height,
                ),
                properties: Some(PropertiesMsg {
                    prop_json: serde_json::to_string(&self.props).unwrap(),
                }),
            }),
            obj_data: Vec::new(),
        }
    }

    async fn client_representation(
        &self,
        conn: &mut GeomKernel,
    ) -> Result<UpdateOutputMsg, OpsError> {
        let input = MakePrismInput {
            first_pt: to_pt_msg(&self.first_pt),
            second_pt: to_pt_msg(&self.second_pt),
            width: self.props.width,
            height: self.props.height,
        };
        let output = conn 
            .make_prism(Request::new(input))
            .await?
            .into_inner();
        Ok(UpdateOutputMsg {
            output: Some(update_output_msg::Output::Mesh(MeshDataMsg {
                positions: output.positions,
                indices: output.indices,
                meta_json: serde_json::to_string(&self.props)?,
            })),
            views: None,
        })
    }

    fn set_result(
        &mut self,
        ref_type: ref_id_msg::RefType,
        index: u64,
        result: Option<ResultInfo>,
    ) {
    }
}
