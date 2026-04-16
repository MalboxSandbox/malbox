//! Runtime implementations that run a [`Plugin`](crate::plugin::Plugin)
//! against a transport.
//!
//! Two runtimes are provided:
//!
//! - [`guest`] — gRPC server runtime for plugins running inside a VM.
//!   Gated behind the `guest` feature.
//! - [`host`] — iceoryx2 IPC runtime for plugins running on the daemon host.
//!   Gated behind the `host` feature.
//!
//! Plugin authors don't usually need to import from this module — the
//! `#[malbox::host_plugin]` and `#[malbox::guest_plugin]` macros generate
//! a `fn main()` that constructs the appropriate runtime.

#[cfg(feature = "guest")]
pub mod guest;

#[cfg(feature = "host")]
pub mod host;

#[cfg(feature = "guest")]
pub use guest::{GuestPluginRuntime, GuestRuntimeConfig};

#[cfg(feature = "host")]
pub use host::HostRuntime;
