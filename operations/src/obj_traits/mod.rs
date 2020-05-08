use crate::*;

mod utils;
pub use utils::*;

pub enum ResultInfo {
    Exists,
    Visibile(bool),
    AxisAlignedBBox(AxisAlignedBBoxMsg),
    Point(Point3Msg),
    Line(LineMsg),
    Plane(PlaneMsg),
    Property(serde_json::Value),
}

#[tonic::async_trait]
pub trait Data: std::fmt::Debug + Send + Sync + Sized {
    fn from_object_msg(id: String, msg: &ObjectMsg) -> Result<Self, OpsError>;
    fn to_object_msg(self) -> ObjectMsg;

    async fn client_representation(
        &self,
        conn: &mut GeomKernel,
    ) -> Result<UpdateOutputMsg, OpsError>;

    fn set_result(&mut self, ref_type: ref_id_msg::RefType, index: u64, result: Option<ResultInfo>);
}
