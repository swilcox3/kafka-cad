pub mod geom_kernel {
    tonic::include_proto!("geom_kernel");
}

pub type GeomKernelClient =
    geom_kernel::geometry_kernel_client::GeometryKernelClient<tonic::transport::Channel>;
pub use geom_kernel::{MakePrismInput, MakePrismOutput, Point3Msg};

pub async fn make_prism(
    client: &mut GeomKernelClient,
    input: MakePrismInput,
) -> Result<tonic::Response<MakePrismOutput>, tonic::Status> {
    let response = client.make_prism(tonic::Request::new(input)).await?;
    Ok(response)
}

pub async fn connect(url: String) -> Result<GeomKernelClient, Box<dyn std::error::Error>> {
    let client = GeomKernelClient::connect(url).await?;
    Ok(client)
}
