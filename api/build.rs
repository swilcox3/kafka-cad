fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(
            &[
                "../proto/api.proto",
                "../proto/geom.proto",
                "../proto/object_state.proto",
                "../proto/operations.proto",
                "../proto/objects.proto",
                "../proto/undo.proto",
                "../proto/representation.proto",
                "../proto/submit.proto",
            ],
            &["../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
