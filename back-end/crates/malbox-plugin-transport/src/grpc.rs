//! Shared gRPC types: protobuf definitions and event conversions.
//!
//! The daemon-side client lives in [`crate::daemon`] and the plugin-side
//! server/emitter/receiver live in [`crate::plugin`].

pub mod conversions;

/// Generated protobuf types and service definitions.
#[expect(
    missing_docs,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason,
    clippy::as_conversions,
    clippy::clone_on_ref_ptr,
    clippy::default_trait_access,
    clippy::doc_markdown,
    clippy::exhaustive_enums,
    clippy::missing_errors_doc,
    clippy::too_many_lines,
    reason = "tonic/prost-generated code is not hand-editable"
)]
pub mod proto {
    tonic::include_proto!("malbox.plugin");
}
