//! Worker pool implementation.
//!
//! Spawns and manages a pool of Worker actors as tokio tasks.

use super::Worker;
use crate::task::queue::TaskQueue;
use crate::task::store::TaskStore;
use crate::worker::event::WorkerEvent;
use malbox_plugin_internal::manager::PluginManager;
use malbox_resources::{MachinePool, ResolvedTransport};
use malbox_utils::{ResultStore, SampleStore};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{error, info};

/// A managed worker with its shutdown handle and join handle.
struct ManagedWorker {
    shutdown_tx: Option<oneshot::Sender<()>>,
    join_handle: JoinHandle<()>,
}

/// Pool of workers for task execution.
pub struct WorkerPool {
    workers: Vec<ManagedWorker>,
    max_workers: usize,
}

impl WorkerPool {
    /// Create a new empty worker pool.
    pub fn new(max_workers: usize) -> Self {
        Self {
            workers: Vec::with_capacity(max_workers),
            max_workers,
        }
    }

    /// Spawn all workers in the pool.
    ///
    /// Each worker is an independent tokio task that competes
    /// for tasks from the shared queue.
    pub fn spawn_workers(
        &mut self,
        task_queue: Arc<TaskQueue>,
        task_store: Arc<TaskStore>,
        machine_pool: Arc<MachinePool>,
        plugin_manager: Arc<PluginManager>,
        event_tx: mpsc::Sender<WorkerEvent>,
        transport: Option<Arc<ResolvedTransport>>,
        sample_store: Arc<SampleStore>,
        result_store: Arc<ResultStore>,
    ) {
        for _ in 0..self.max_workers {
            let (shutdown_tx, shutdown_rx) = oneshot::channel();

            let worker = Worker::new(
                Arc::clone(&task_queue),
                Arc::clone(&task_store),
                Arc::clone(&machine_pool),
                Arc::clone(&plugin_manager),
                event_tx.clone(),
                transport.clone(),
                Arc::clone(&sample_store),
                Arc::clone(&result_store),
            );

            let worker_id = worker.id().clone();
            info!(worker_id = %worker_id, "Spawning worker");

            let join_handle = tokio::spawn(worker.run(shutdown_rx));

            self.workers.push(ManagedWorker {
                shutdown_tx: Some(shutdown_tx),
                join_handle,
            });
        }

        info!(count = self.workers.len(), "Worker pool initialized");
    }

    /// Get the number of active workers.
    pub fn active_count(&self) -> usize {
        self.workers
            .iter()
            .filter(|w| !w.join_handle.is_finished())
            .count()
    }

    /// Get the maximum number of workers.
    pub fn max_workers(&self) -> usize {
        self.max_workers
    }

    /// Shutdown all workers gracefully.
    pub async fn shutdown(&mut self) {
        info!("Shutting down worker pool");

        // Send shutdown signal to all workers
        for managed in &mut self.workers {
            if let Some(tx) = managed.shutdown_tx.take() {
                let _ = tx.send(());
            }
        }

        // Wait for all workers to finish
        for managed in &mut self.workers {
            if let Err(e) = (&mut managed.join_handle).await {
                error!(error = %e, "Worker task panicked during shutdown");
            }
        }

        self.workers.clear();
        info!("Worker pool shut down");
    }
}
