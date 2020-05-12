fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile(
            &["../../proto/geom_kernel.proto", "../../proto/geom.proto"],
            &["../../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
