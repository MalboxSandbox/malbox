use malbox_config::Config;
use malbox_database::PgPool;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info};

pub mod error;
pub mod resource;
pub mod scheduler;
pub mod task;
pub mod worker;

// Re-export common types
pub use error::{Result, SchedulerError, TaskError, WorkerError};
pub use task::{PluginContext, PluginResult, PluginStatus, ResourceAllocation, TaskResult};

pub async fn init_scheduler() {
    todo!()
}
