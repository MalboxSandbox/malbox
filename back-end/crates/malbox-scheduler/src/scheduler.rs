//! Scheduler implementation.
//!
//! The Scheduler initializes shared state (TaskQueue, TaskStore, WorkerPool),
//! loads pending tasks from the database on startup, ingests new tasks from
//! the HTTP layer, and spawns Worker actors to process them.

use crate::error::Result;
use crate::task::cancel::TaskCancellationRegistry;
use crate::task::queue::TaskQueue;
use crate::task::store::TaskStore;
use crate::worker::event::WorkerEvent;
use crate::worker::pool::WorkerPool;
use malbox_database::PgPool;
use malbox_database::repositories::tasks::Task;
use malbox_plugin_internal::manager::PluginManager;
use malbox_resources::{MachinePool, ResolvedTransport};
use malbox_utils::{ResultStore, SampleStore};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// The scheduler orchestrates task ingestion, queuing, and worker management.
pub struct Scheduler {
    task_queue: Arc<TaskQueue>,
    task_store: Arc<TaskStore>,
    machine_pool: Arc<MachinePool>,
    plugin_manager: Arc<PluginManager>,
    worker_pool: Arc<WorkerPool>,
    max_workers: usize,
    min_workers: usize,
    transport: Option<Arc<ResolvedTransport>>,
    sample_store: Arc<SampleStore>,
    result_store: Arc<ResultStore>,
    cancel_registry: Arc<TaskCancellationRegistry>,
    #[allow(dead_code)]
    token: CancellationToken,
}

impl Scheduler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
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
    ) -> Self {
        let idle_timeout = std::time::Duration::from_millis(idle_timeout_ms);
        let cancel_registry = Arc::new(TaskCancellationRegistry::new());
        Self {
            task_queue: Arc::new(TaskQueue::new()),
            task_store: Arc::new(TaskStore::new(db_pool)),
            machine_pool,
            plugin_manager,
            worker_pool: Arc::new(WorkerPool::new(
                max_workers,
                min_workers,
                idle_timeout,
                token.child_token(),
            )),
            max_workers,
            min_workers,
            transport,
            sample_store,
            result_store,
            cancel_registry,
            token,
        }
    }

    pub fn cancel_registry(&self) -> Arc<TaskCancellationRegistry> {
        Arc::clone(&self.cancel_registry)
    }

    pub async fn run(
        self,
        mut task_rx: mpsc::Receiver<Task>,
        token: CancellationToken,
    ) -> Result<()> {
        // 1. Reset tasks stranded in transient states from a prior daemon run.
        match self.task_store.recover_orphaned_tasks().await {
            Ok(0) => {}
            Ok(count) => warn!(count, "Reset orphaned tasks from prior run to 'failed'"),
            Err(e) => error!(error = %e, "Failed to reset orphaned tasks"),
        }

        // 2. Load pending tasks from DB.
        let recovered_count = match self.task_store.load_pending_tasks().await {
            Ok(pending) => {
                let count = pending.len();
                if count > 0 {
                    info!(count, "Recovered pending tasks from database");
                    let entries: Vec<(i32, i64)> = pending
                        .iter()
                        .filter_map(|t| t.id.map(|id| (id, t.priority)))
                        .collect();
                    self.task_queue.enqueue_batch(entries).await;
                }
                count
            }
            Err(e) => {
                error!(error = %e, "Failed to load pending tasks from database");
                0
            }
        };

        // 3. Create worker event channel.
        let (event_tx, mut event_rx) = mpsc::channel::<WorkerEvent>(100);

        // 4. Spawn the baseline pool.
        self.worker_pool.spawn_initial(
            Arc::clone(&self.task_queue),
            Arc::clone(&self.task_store),
            Arc::clone(&self.machine_pool),
            Arc::clone(&self.plugin_manager),
            event_tx,
            self.transport.clone(),
            Arc::clone(&self.sample_store),
            Arc::clone(&self.result_store),
            Arc::clone(&self.cancel_registry),
        );

        debug!(
            max_workers = self.max_workers,
            min_workers = self.min_workers,
            "Scheduler run loop entered"
        );

        // 5. For each recovered pending task (beyond the baseline), give the
        //    pool a chance to scale up. `ensure_capacity` caps internally.
        if recovered_count > self.min_workers {
            let additional = recovered_count - self.min_workers;
            for _ in 0..additional {
                self.worker_pool.ensure_capacity();
            }
        }

        let task_queue = Arc::clone(&self.task_queue);
        let task_store = Arc::clone(&self.task_store);
        let worker_pool = Arc::clone(&self.worker_pool);

        // 6. Spawn task ingestion loop.
        let ingest_handle = tokio::spawn(async move {
            while let Some(task) = task_rx.recv().await {
                let task_id = task.id.unwrap_or(-1);
                let priority = task.priority;

                task_store.cache_task(task).await;
                task_queue.enqueue(task_id, priority).await;
                worker_pool.ensure_capacity();

                debug!(task_id, priority, "Task ingested into queue");
            }
            info!("Task ingestion channel closed");
        });

        // 7. Spawn worker event listener.
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

        // 8. Wait for shutdown signal.
        token.cancelled().await;
        info!("Scheduler received shutdown signal");

        self.worker_pool.shutdown().await;
        ingest_handle.abort();
        event_handle.abort();

        info!("Scheduler stopped");
        Ok(())
    }
}
