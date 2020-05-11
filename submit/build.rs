fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .extern_path(".geom", "::geom")
        .extern_path(".object_state", "::object_state")
        .compile(
            &[
                "../proto/object_state.proto",
                "../proto/objects.proto",
                "../proto/dependencies.proto",
                "../proto/representation.proto",
                "../proto/submit.proto",
                "../proto/operations.proto",
            ],
            &["../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
