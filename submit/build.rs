fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(
            &[
                "../proto/object_state.proto",
                "../proto/objects.proto",
                "../proto/geom.proto",
                "../proto/dependencies.proto",
                "../proto/representation.proto",
                "../proto/submit.proto",
                "../proto/operations.proto",
            ],
            &["../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
