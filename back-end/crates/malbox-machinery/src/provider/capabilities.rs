//! Capability traits for providers.
//!
//! Capabilities are independent, composable traits that providers can implement.
//! Each capability represents a specific functionality that a provider supports.
//!
//! Providers implement only the capabilities they support, and strategies
//! (managers) depend on specific capabilities rather than entire providers.

pub mod allocate;
pub mod clone;
pub mod guest_access;
pub mod migrate;
pub mod snapshot;

// Re-export all capability traits and types
pub use allocate::Allocate;
pub use clone::Clone;
pub use guest_access::{
    ExecOptions, ExecResult, GuestAccess, GuestSession, GuestStatus, TomlValue,
};
pub use migrate::Migrate;
pub use snapshot::{Snapshot, SnapshotId, SnapshotInfo};
