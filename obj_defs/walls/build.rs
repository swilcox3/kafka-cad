fn main() {
    tonic_build::configure()
        .build_client(false)
        .build_server(true)
        .compile(
            &[
                "../../proto/walls.proto",
                "../../proto/object_state.proto",
                "../../proto/representation.proto",
                "../../proto/obj_defs.proto",
            ],
            &["../../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
