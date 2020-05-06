use crate::*;
use tonic::transport::Channel;
use tonic::Status;

pub type GeomKernel = geometry_kernel_client::GeometryKernelClient<Channel>;

pub async fn get_triangles(
    geom_kernel: &mut GeomKernel,
    first_pt: Point3Msg,
    second_pt: Point3Msg,
    width: f64,
    height: f64,
) -> Result<MakePrismOutput, Status> {
    let input = MakePrismInput {
        first_pt: Some(first_pt),
        second_pt: Some(second_pt),
        width,
        height,
    };
    let output = geom_kernel
        .make_prism(Request::new(input))
        .await?
        .into_inner();
    Ok(output)
}
