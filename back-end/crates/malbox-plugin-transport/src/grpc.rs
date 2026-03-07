//! Shared gRPC types: protobuf definitions and event conversions.
//!
//! The daemon-side client lives in [`crate::daemon`] and the plugin-side
//! server/emitter/receiver live in [`crate::plugin`].

pub mod conversions;

/// Generated protobuf types and service definitions.
pub mod proto {
    tonic::include_proto!("malbox.plugin");
}
