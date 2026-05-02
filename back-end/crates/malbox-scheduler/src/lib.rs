//! Malbox Scheduler
//!
//! Task scheduling and worker pool management using Actor-per-Worker pattern.
//! Workers are independent tokio tasks that compete for tasks from a shared
//! priority queue.

use malbox_database::PgPool;
use malbox_database::repositories::tasks::Task;
use malbox_plugin_internal::manager::PluginManager;
use malbox_resources::{MachinePool, ResolvedTransport};
use malbox_utils::{ResultStore, SampleStore};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub mod error;
pub mod scheduler;
pub mod task;
pub mod worker;

// Re-export common types
pub use error::{Result, SchedulerError, TaskError, WorkerError};
pub use task::{
    PluginContext, PluginResult, PluginStatus, ResourceAllocation, TaskCancellationRegistry,
    TaskResult,
};

/// Initialize and start the scheduler.
///
/// Returns a sender for submitting tasks and a shutdown sender.
/// The scheduler runs in a background tokio task.
#[allow(clippy::too_many_arguments)]
pub async fn init_scheduler(
    db_pool: PgPool,
    machine_pool: Arc<MachinePool>,
    plugin_manager: Arc<PluginManager>,
    max_workers: usize,
    min_workers: usize,
    idle_timeout_ms: u64,
    transport: Option<Arc<ResolvedTransport>>,
    sample_store: Arc<SampleStore>,
    result_store: Arc<ResultStore>,
    token: CancellationToken,
) -> Result<(mpsc::Sender<Task>, Arc<TaskCancellationRegistry>)> {
    let (task_tx, task_rx) = mpsc::channel::<Task>(100);

    let scheduler = scheduler::Scheduler::new(
        db_pool,
        machine_pool,
        plugin_manager,
        max_workers,
        min_workers,
        idle_timeout_ms,
        transport,
        sample_store,
        result_store,
        token.clone(),
    );
    let cancel_registry = scheduler.cancel_registry();

    tokio::spawn(async move {
        if let Err(e) = scheduler.run(task_rx, token).await {
            error!(error = %e, "Scheduler exited with error");
        }
    });

    info!(
        max_workers,
        min_workers, idle_timeout_ms, "Scheduler started"
    );
    Ok((task_tx, cancel_registry))
}
