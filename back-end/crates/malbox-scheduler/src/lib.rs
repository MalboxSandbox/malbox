//! Malbox Scheduler
//!
//! Task scheduling and worker pool management using Actor-per-Worker pattern.
//! Workers are independent tokio tasks that compete for tasks from a shared
//! priority queue.

use malbox_config::MachineryConfig;
use malbox_database::PgPool;
use malbox_database::repositories::tasks::Task;
use malbox_plugin_internal::manager::PluginManager;
use malbox_resources::{MachineryManager, ResolvedTransport};
use malbox_storage::SampleStore;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::info;

pub mod error;
pub mod scheduler;
pub mod task;
pub mod worker;

// Re-export common types
pub use error::{Result, SchedulerError, TaskError, WorkerError};
pub use task::{PluginContext, PluginResult, PluginStatus, ResourceAllocation, TaskResult};

/// Initialize and start the scheduler.
///
/// Returns a sender for submitting tasks and a shutdown sender.
/// The scheduler runs in a background tokio task.
pub async fn init_scheduler(
    db_pool: PgPool,
    machinery_manager: Arc<dyn MachineryManager>,
    machinery_config: MachineryConfig,
    plugin_manager: Arc<PluginManager>,
    worker_count: usize,
    transport: Option<Arc<ResolvedTransport>>,
    sample_store: Arc<SampleStore>,
) -> Result<(mpsc::Sender<Task>, oneshot::Sender<()>)> {
    info!("Initializing scheduler");

    // Create channels
    let (task_tx, task_rx) = mpsc::channel::<Task>(100);
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // Create and run scheduler in background
    let scheduler = scheduler::Scheduler::new(
        db_pool,
        machinery_manager,
        machinery_config,
        plugin_manager,
        worker_count,
        transport,
        sample_store,
    );

    tokio::spawn(async move {
        if let Err(e) = scheduler.run(task_rx, shutdown_rx).await {
            tracing::error!(error = %e, "Scheduler exited with error");
        }
    });

    info!(worker_count, "Scheduler started");
    Ok((task_tx, shutdown_tx))
}
