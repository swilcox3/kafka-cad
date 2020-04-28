fn main() {
    prost_build::compile_protos(
        &[
            "../../proto/walls.proto",
            "../../proto/object_state.proto",
            "../../proto/representation.proto",
            "../../proto/obj_defs.proto"
        ],
        &["../../proto"],
    )
    .unwrap();
}
