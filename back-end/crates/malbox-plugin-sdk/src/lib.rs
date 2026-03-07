//! Malbox Plugin SDK
//!
//! Developer-facing API for building Malbox analysis plugins.
//! Plugins implement the [`Plugin`] trait and run via [`PluginRuntime`] (IPC host)
//! or [`GuestPluginRuntime`] (gRPC guest, requires `guest` feature).

pub mod error;
pub mod plugin;
pub mod runtime;

#[cfg(feature = "guest")]
pub mod guest_runtime;

/// Convenience re-exports for plugin authors.
pub mod prelude {
    pub use crate::error::{Result, SdkError};
    pub use crate::plugin::{EventContext, Plugin};
    pub use crate::runtime::{PluginRuntime, RuntimeConfig};

    #[cfg(feature = "guest")]
    pub use crate::guest_runtime::{GuestPluginRuntime, GuestRuntimeConfig};

    pub use malbox_plugin_transport::messages::events::{
        DaemonEvent, Event, Payload, PluginEvent, PluginEventPayload, SampleEvent,
        SampleEventPayload, TaskEvent, TaskEventPayload,
    };
}

pub use plugin::{EventContext, Plugin};
pub use runtime::{PluginRuntime, RuntimeConfig};

#[cfg(feature = "guest")]
pub use guest_runtime::{GuestPluginRuntime, GuestRuntimeConfig};
