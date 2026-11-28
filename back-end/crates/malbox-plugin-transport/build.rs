//! Build script: compiles the plugin gRPC protobuf definitions with tonic when
//! the `grpc` feature is enabled.

fn main() {
    #[cfg(feature = "grpc")]
    {
        tonic_prost_build::configure()
            .compile_protos(&["proto/plugin_service.proto"], &["proto"])
            .expect("Failed to compile plugin_service.proto");
    }
}
