fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(
            &[
                "../../proto/geom.proto",
                "../../proto/walls.proto",
                "../../proto/object_state.proto",
                "../../proto/representation.proto",
                "../../proto/obj_defs.proto",
                "../../proto/geom_kernel.proto",
            ],
            &["../../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
