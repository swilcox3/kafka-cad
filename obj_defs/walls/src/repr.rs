use crate::*;
use tonic::transport::Channel;

pub type GeomKernel = geometry_kernel_client::GeometryKernelClient<Channel>;

async fn get_triangles(
    geom_kernel: &mut GeomKernel,
    wall: &Wall,
) -> Result<MakePrismOutput, WallError> {
    let input = MakePrismInput {
        first_pt: Some(wall.first_pt.clone()),
        second_pt: Some(wall.second_pt.clone()),
        width: wall.width,
        height: wall.height,
    };
    let output = geom_kernel
        .make_prism(Request::new(input))
        .await?
        .into_inner();
    Ok(output)
}

pub async fn get_repr(geom_url: String, wall: Wall) -> Result<UpdateOutputMsg, WallError> {
    let mut geom_client = geometry_kernel_client::GeometryKernelClient::connect(geom_url).await?;
    let mesh_data = repr::get_triangles(&mut geom_client, &wall).await?;
    Ok(UpdateOutputMsg {
        output: Some(update_output_msg::Output::Mesh(MeshDataMsg {
            positions: mesh_data.positions,
            indices: mesh_data.indices,
            meta_json: serde_json::to_string(&wall.get_props())?,
        })),
        views: None,
    })
}
