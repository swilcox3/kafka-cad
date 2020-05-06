fn main() {
    let mut config = prost_build::Config::new();
    config.extern_path(".geom", "::geom");
    config
        .compile_protos(&["../../proto/object_state.proto"], &["../../proto"])
        .unwrap_or_else(|e| panic!("Failed to compile protos {:?}", e));
}
