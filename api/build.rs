fn main() {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(
            &[
                "../proto/geom.proto",
                "../proto/api.proto",
                "../proto/walls.proto",
                "../proto/undo.proto",
                "../proto/object_state.proto",
                "../proto/obj_defs.proto",
                "../proto/representation.proto",
                "../proto/submit.proto",
            ],
            &["../proto"],
        )
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
