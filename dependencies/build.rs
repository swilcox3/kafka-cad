fn main() {
    tonic_build::configure()
        .build_client(false)
        .build_server(true)
        .compile(&["../proto/dependencies.proto"], &["../proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
