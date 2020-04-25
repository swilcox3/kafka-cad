fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(
            &[
                "../proto/undo.proto",
                "../proto/object_state.proto",
                "../proto/objects.proto",
                "../proto/submit.proto",
            ],
            &["../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
