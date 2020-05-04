use tonic::transport::Channel;
use tonic::Status;

mod geom_kernel {
    tonic::include_proto!("geom_kernel");
}

pub type GeomKernel = geom_kernel::geometry_kernel_client::GeometryKernelClient<Channel>;

pub fn get_triangles(geom_kernel: &mut GeomKernel, )
