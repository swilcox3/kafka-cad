fn main() {
    tonic_build::configure()
        .build_client(false)
        .build_server(true)
        .compile(
            &[
                "../../proto/operations.proto",
                "../../proto/representation.proto",
                "../../proto/object_state.proto",
                "../../proto/geom.proto",
            ],
            &["../../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
