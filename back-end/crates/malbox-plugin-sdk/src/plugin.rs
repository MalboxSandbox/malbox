//! Core plugin traits.
//!
//! Every plugin implements [`Plugin`] (health checks). From there, the
//! two deployment models each have their own trait:
//!
//! - [`HostPlugin`] - runs on the daemon host, receives tasks and events
//!   over IPC.
//! - [`GuestPlugin`] - runs inside an analysis VM, follows a linear
//!   start/execute/stop lifecycle over gRPC.

pub mod guest;
pub mod host;

pub use guest::{GuestPlugin, LaunchResult, default_launch};
pub use host::HostPlugin;

use crate::health::HealthStatus;

/// Base trait shared by all Malbox plugins (host and guest).
///
/// The only method is [`health_check`](Plugin::health_check), which
/// defaults to reporting ready. Override it if your plugin needs warm-up
/// time or depends on an external resource.
pub trait Plugin: Send + Sync + 'static {
    /// Return the plugin's current health status. Defaults to ready.
    fn health_check(&self) -> HealthStatus {
        HealthStatus::ready()
    }
}
