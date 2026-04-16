//! Domain types used throughout the SDK.
//!
//! These are the data types plugin authors work with: metadata, tasks,
//! results, health, and command execution. Each type has its own file
//! and tests.

mod exec;
mod health;
mod meta;
mod result;
mod task;

pub use exec::{ExecRequest, ExecResult, ExecutionInfo};
pub use health::HealthStatus;
pub use meta::{ExecutionContext, PluginMeta, PluginState, PluginType};
pub use result::PluginResult;
pub use task::Task;
