fn main() {
    tonic_prost_build::configure()
        .compile_protos(&["proto/plugin_service.proto"], &["proto"])
        .expect("Failed to compile plugin_service.proto");
}
