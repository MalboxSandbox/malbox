//! C-ABI event types and Rust<->C conversion functions.
//!
//! These types mirror the `malbox_plugin_transport::messages::events` types
//! in a form that can cross the C ABI boundary.  The flat `MalboxEvent` struct
//! uses a discriminant tag and an integer ID field for the variable payload.

pub mod convert;
pub mod types;

pub use types::{MalboxEvent, MalboxEventTag};

pub(crate) use convert::{c_event_to_rust, rust_event_to_c};
