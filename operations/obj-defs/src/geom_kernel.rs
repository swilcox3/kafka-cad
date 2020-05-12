use crate::*;

mod geom {
    tonic::include_proto!("geom");
}

mod geom_kernel {
    tonic::include_proto!("geom_kernel");
}
use geom_kernel::*;
use geometry_kernel_client::GeometryKernelClient;
use geom::*;


fn to_pt_msg(pt: &Point3f) -> Point3Msg {
    Point3Msg {
        x: pt.x,
        y: pt.y,
        z: pt.z,
    }
}

#[derive(Clone)]
pub struct GeomConn {
    conn: GeometryKernelClient<tonic::transport::Channel>,
}

pub async fn new_geom_conn(url: String) -> Result<GeomConn, ObjError> {
    Ok(GeomConn {
        conn: GeometryKernelClient::connect(url).await?,
    })
}

#[async_trait::async_trait]
impl GeomKernel for GeomConn {
    async fn make_prism(
        &mut self,
        first_pt: &Point3f,
        second_pt: &Point3f,
        width: f64,
        height: f64,
        results: &mut MeshData,
    ) -> Result<(), ObjError> {
        let input = MakePrismInput {
            first_pt: Some(to_pt_msg(first_pt)),
            second_pt: Some(to_pt_msg(second_pt)),
            width,
            height,
        };
        let resp = self.conn.make_prism(input)
            .await?;
        let output = resp.into_inner();
        results.positions = output.positions;
        results.indices = output.indices;
        Ok(())
    }
}
