fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .extern_path(".geom", "::geom")
        .extern_path(".object_state", "::object_state")
        .compile(
            &[
                "../../proto/walls.proto",
                "../../proto/representation.proto",
                "../../proto/obj_defs.proto",
                "../../proto/geom_kernel.proto",
            ],
            &["../../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
