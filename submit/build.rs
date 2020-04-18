fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(
            &[
                "../proto/submit.proto",
                "../proto/object_state.proto",
                "../proto/dependencies.proto",
            ],
            &["../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
