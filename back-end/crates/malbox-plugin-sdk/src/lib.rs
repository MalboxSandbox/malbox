//! Malbox Plugin SDK
//!
//! Macro-driven framework for building Malbox analysis plugins.
//!
//! Plugin authors annotate a struct with [`#[malbox::host_plugin]`](macro@host_plugin)
//! or [`#[malbox::guest_plugin]`](macro@guest_plugin) and add handler methods
//! with [`#[malbox::on_task]`](macro@on_task),
//! [`#[malbox::on_start]`](macro@on_start),
//! [`#[malbox::on_stop]`](macro@on_stop), and
//! [`#[malbox::on_event(...)]`](macro@on_event).
//!
//! Package metadata (`name`, `version`, `description`, `authors`) is read
//! automatically from the plugin's `Cargo.toml` — only `state` and `execution`
//! need to be specified in the `#[malbox(...)]` attribute.
//!
//! # Example
//!
//! ```ignore
//! extern crate malbox_plugin_sdk as malbox;
//! use malbox::prelude::*;
//!
//! #[malbox::host_plugin]
//! #[malbox(state = "persistent", execution = "parallel")]
//! struct MyPlugin;
//!
//! #[malbox::handlers]
//! impl MyPlugin {
//!     #[malbox::on_task]
//!     fn analyze(&self, task: Task, ctx: &Context) -> Result<Vec<PluginResult>> {
//!         Ok(vec![])
//!     }
//! }
//! ```

pub mod context;
pub mod error;
pub mod types;

#[doc(hidden)]
pub mod internal;

// Feature-gated runtime modules
#[cfg(feature = "host")]
pub mod host_runtime;

#[cfg(feature = "guest")]
pub mod guest_runtime;

pub mod build;

/// Convenience re-exports for plugin authors.
pub mod prelude {
    pub use crate::context::Context;
    pub use crate::error::{Result, SdkError};
    pub use crate::types::{
        ExecutionContext, HealthStatus, PluginMeta, PluginResult, PluginState, PluginType, Task,
    };

    // Re-export common dependencies so plugin authors don't need them in Cargo.toml
    pub use serde::Deserialize;
    pub use tracing::{debug, error, info, warn};

    // Re-export transport event types for on_event handlers
    pub use malbox_plugin_transport::messages::events::{
        DaemonEvent, Event, Payload, PluginEvent, PluginEventPayload, SampleEvent,
        SampleEventPayload, TaskEvent, TaskEventPayload,
    };
}

// Re-export macros so `malbox::host_plugin` works
pub use malbox_plugin_macros::*;

// Top-level re-exports
pub use context::Context;
pub use error::{Result, SdkError};
pub use types::{HealthStatus, PluginMeta, PluginResult, Task};
