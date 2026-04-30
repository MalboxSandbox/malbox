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
//!     fn analyze(&self, task: Task, ctx: &Context) -> Result<()> {
//!         Ok(())
//!     }
//! }
//! ```

pub mod context;
pub mod error;
pub mod guest_plugin;
pub mod log;
pub mod plugin;
pub mod runtime;
pub mod stash;
pub mod types;

#[doc(hidden)]
pub mod internal;

pub mod build;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

/// Convenience re-exports for plugin authors.
pub mod prelude {
    pub use crate::context::Context;
    pub use crate::error::{Result, SdkError};
    pub use crate::guest_plugin::GuestPlugin;
    pub use crate::plugin::HostPlugin;
    pub use crate::types::{
        ArtifactRef, Block, CalloutLevel, Classification, Column, Confidence, ExecutionContext,
        GraphEdge, GraphNode, HealthStatus, Indicator, KvPair, PluginInfo, PluginMeta,
        PluginResult, PluginState, PluginType, REPORT_RESULT_NAME, Report, ReportBuilder,
        SCHEMA_VERSION, Section, SectionBuilder, Task, TimelineEvent, TreeNode, Ttp, Verdict,
    };

    // Re-export common dependencies so plugin authors don't need them in Cargo.toml
    pub use serde::{Deserialize, Serialize};
    pub use tracing::{debug, error, info, warn};

    // Re-export transport event type for on_event handlers
    pub use malbox_plugin_transport::messages::events::Event;
}

// Re-export macros so `malbox::host_plugin` works
pub use malbox_plugin_macros::*;

// Top-level re-exports
pub use context::Context;
pub use error::{Result, SdkError};
pub use guest_plugin::GuestPlugin;
pub use plugin::HostPlugin;
pub use types::{
    HealthStatus, PluginMeta, PluginResult, REPORT_RESULT_NAME, Report, ReportBuilder,
    SCHEMA_VERSION, Task,
};

#[cfg(test)]
mod prelude_tests {
    #[test]
    fn prelude_exports_serialize_and_deserialize() {
        // This test only needs to compile. It verifies that both Serialize and
        // Deserialize are reachable via `malbox_plugin_sdk::prelude::*`.
        use crate::prelude::*;

        #[derive(Serialize, Deserialize)]
        struct Foo {
            x: i32,
        }

        let _ = Foo { x: 1 };
    }
}
