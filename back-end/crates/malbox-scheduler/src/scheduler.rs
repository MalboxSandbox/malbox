//! Scheduler implementation.
//!
//! The Scheduler initializes shared state (TaskQueue, TaskStore, WorkerPool),
//! loads pending tasks from the database on startup, ingests new tasks from
//! the HTTP layer, and spawns Worker actors to process them.

use crate::error::Result;
use crate::task::queue::TaskQueue;
use crate::task::store::TaskStore;
use crate::worker::event::WorkerEvent;
use crate::worker::pool::WorkerPool;
use malbox_config::MachineryConfig;
use malbox_database::PgPool;
use malbox_database::repositories::tasks::Task;
use malbox_plugin_internal::manager::PluginManager;
use malbox_resources::{MachinePool, ResolvedTransport};
use malbox_utils::SampleStore;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

/// The scheduler orchestrates task ingestion, queuing, and worker management.
pub struct Scheduler {
    task_queue: Arc<TaskQueue>,
    task_store: Arc<TaskStore>,
    machine_pool: Arc<MachinePool>,
    machinery_config: MachineryConfig,
    plugin_manager: Arc<PluginManager>,
    worker_pool: WorkerPool,
    worker_count: usize,
    transport: Option<Arc<ResolvedTransport>>,
    sample_store: Arc<SampleStore>,
}

impl Scheduler {
    /// Create a new scheduler.
    pub fn new(
        db_pool: PgPool,
        machine_pool: Arc<MachinePool>,
        machinery_config: MachineryConfig,
        plugin_manager: Arc<PluginManager>,
        worker_count: usize,
        transport: Option<Arc<ResolvedTransport>>,
        sample_store: Arc<SampleStore>,
    ) -> Self {
        Self {
            task_queue: Arc::new(TaskQueue::new()),
            task_store: Arc::new(TaskStore::new(db_pool)),
            machine_pool,
            machinery_config,
            plugin_manager,
            worker_pool: WorkerPool::new(worker_count),
            worker_count,
            transport,
            sample_store,
        }
    }

    /// Run the scheduler.
    ///
    /// This method:
    /// 1. Loads pending tasks from the database
    /// 2. Spawns an ingestion task that reads from the task receiver
    /// 3. Spawns workers via the WorkerPool
    /// 4. Spawns a worker event listener
    /// 5. Waits for shutdown signal
    pub async fn run(
        mut self,
        mut task_rx: mpsc::Receiver<Task>,
        shutdown_rx: oneshot::Receiver<()>,
    ) -> Result<()> {
        // 1. Load pending tasks from DB
        match self.task_store.load_pending_tasks().await {
            Ok(pending) => {
                if !pending.is_empty() {
                    info!(
                        count = pending.len(),
                        "Recovered pending tasks from database"
                    );
                    let entries: Vec<(i32, i64)> = pending
                        .iter()
                        .filter_map(|t| t.id.map(|id| (id, t.priority)))
                        .collect();
                    self.task_queue.enqueue_batch(entries).await;
                }
            }
            Err(e) => {
                error!(error = %e, "Failed to load pending tasks from database");
            }
        }

        // 2. Create worker event channel
        let (event_tx, mut event_rx) = mpsc::channel::<WorkerEvent>(100);

        // 3. Spawn workers
        self.worker_pool.spawn_workers(
            Arc::clone(&self.task_queue),
            Arc::clone(&self.task_store),
            Arc::clone(&self.machine_pool),
            self.machinery_config.clone(),
            Arc::clone(&self.plugin_manager),
            event_tx,
            self.transport.clone(),
            Arc::clone(&self.sample_store),
        );

        info!(workers = self.worker_count, "Scheduler running");

        let task_queue = Arc::clone(&self.task_queue);
        let task_store = Arc::clone(&self.task_store);

        // 4. Spawn task ingestion loop
        let ingest_handle = tokio::spawn(async move {
            while let Some(task) = task_rx.recv().await {
                // Should throw error instead of putting arbitrary `-1`.
                let task_id = task.id.unwrap_or(-1);
                let priority = task.priority;

                // Cache the task in the store
                task_store.cache_task(task).await;

                // Enqueue for workers to pick up
                task_queue.enqueue(task_id, priority).await;
                info!(task_id, priority, "Task ingested into queue");
            }
            info!("Task ingestion channel closed");
        });

        // 5. Spawn worker event listener
        let event_handle = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match &event {
                    WorkerEvent::JobCompleted {
                        worker_id,
                        duration,
                        ..
                    } => {
                        info!(worker_id = %worker_id, duration = ?duration, "Job completed");
                    }
                    WorkerEvent::BatchCompleted {
                        worker_id,
                        batch_size,
                        successful_count,
                        duration,
                    } => {
                        info!(
                            worker_id = %worker_id,
                            batch_size,
                            successful_count,
                            duration = ?duration,
                            "Batch completed"
                        );
                    }
                    WorkerEvent::WorkerShutdown { worker_id, reason } => {
                        info!(worker_id = %worker_id, reason = ?reason, "Worker shut down");
                    }
                    WorkerEvent::WorkerError { worker_id, error } => {
                        error!(worker_id = %worker_id, error = %error, "Worker error");
                    }
                }
            }
        });

        // 6. Wait for shutdown signal
        let _ = shutdown_rx.await;
        info!("Scheduler received shutdown signal");

        // Graceful shutdown
        self.worker_pool.shutdown().await;
        ingest_handle.abort();
        event_handle.abort();

        info!("Scheduler stopped");
        Ok(())
    }
}
