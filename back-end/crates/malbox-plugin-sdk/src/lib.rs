//! Malbox Plugin SDK
//!
//! This crate is the foundation for building plugins for Malbox.
//!
//! Plugins come in two flavors:
//!
//! - **Host plugins** run on the daemon and communicate over IPC. Mark your
//!   struct with [`#[malbox::host_plugin]`](macro@host_plugin).
//! - **Guest plugins** run inside a sandboxed environment and communicate over gRPC.
//!   Mark your struct with [`#[malbox::guest_plugin]`](macro@guest_plugin).
//!
//! In both cases, you write handler methods and annotate them with
//! [`#[malbox::on_task]`](macro@on_task),
//! [`#[malbox::on_start]`](macro@on_start),
//! [`#[malbox::on_stop]`](macro@on_stop), or
//! [`#[malbox::on_event(...)]`](macro@on_event).
//! The SDK generates the boilerplate: trait impls, `fn main`,
//! runtime setup, and result transport.
//!
//! Package metadata (`name`, `version`, `description`, `authors`) is read
//! automatically from your `Cargo.toml`, so you only need to specify
//! `state` and `execution` in the attribute.
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
//!     fn analyze(&self, ctx: &Context) -> Result<()> {
//!         Ok(())
//!     }
//! }
//! ```

pub mod context;
pub mod error;
pub mod health;
pub mod meta;
pub mod plugin;
pub mod report;
pub mod result;
pub mod runtime;

#[doc(hidden)]
pub mod internal;
#[doc(hidden)]
pub mod log;
#[cfg(feature = "guest")]
pub(crate) mod stash;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

/// Everything a plugin needs in scope. Start with `use malbox::prelude::*;`.
pub mod prelude {
    pub use crate::context::{Context, ResultSink, TaskInfo};
    pub use crate::error::{Result, SdkError};
    pub use crate::health::HealthStatus;
    pub use crate::meta::{ExecutionContext, PluginMeta, PluginState};
    pub use crate::plugin::{GuestPlugin, HostPlugin, LaunchResult, Plugin};
    pub use crate::report::{
        ArtifactRef, Block, CalloutLevel, Classification, Column, Confidence, GraphEdge, GraphNode,
        Indicator, KvPair, PluginInfo, REPORT_RESULT_NAME, Report, ReportBuilder, SCHEMA_VERSION,
        Section, SectionBuilder, TimelineEvent, TreeNode, Ttp, Verdict,
    };
    pub use crate::result::PluginResult;

    pub use serde::{Deserialize, Serialize};
    pub use tracing::{debug, error, info, warn};

    pub use malbox_plugin_transport::messages::events::Event;
}

pub use malbox_plugin_macros::*;

pub use context::{Context, ResultSink, TaskInfo};
pub use error::{Result, SdkError};
pub use health::HealthStatus;
pub use meta::{ExecutionContext, PluginMeta, PluginState};
pub use plugin::{GuestPlugin, HostPlugin, LaunchResult, Plugin};
pub use report::{
    REPORT_RESULT_NAME, Report, ReportBuilder, SCHEMA_VERSION, SectionBuilder, Verdict,
};
pub use result::PluginResult;
