//! Domain types used throughout the SDK.
//!
//! These are the data types plugin authors work with: metadata, tasks,
//! results, health, and command execution. Each type has its own file
//! and tests.

mod health;
mod meta;
pub mod report;
mod result;
mod task;

pub use health::HealthStatus;
pub use meta::{ExecutionContext, PluginMeta, PluginState, PluginType};
pub use report::{
    ArtifactRef, Block, CalloutLevel, Classification, Column, Confidence, GraphEdge, GraphNode,
    Indicator, KvPair, PluginInfo, REPORT_RESULT_NAME, Report, ReportBuilder, SCHEMA_VERSION,
    Section, SectionBuilder, TimelineEvent, TreeNode, Ttp, Verdict,
};
pub use result::PluginResult;
pub use task::Task;
