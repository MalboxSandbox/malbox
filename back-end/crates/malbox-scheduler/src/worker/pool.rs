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
use std::sync::Mutex as StdMutex;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

/// Decide whether `ensure_capacity` should spawn another worker.
///
/// Returns true iff every currently-active worker is busy AND we are
/// below the configured ceiling.
fn should_spawn(busy: usize, active: usize, max: usize) -> bool {
    busy >= active && active < max
}

/// A managed worker with its shutdown handle and join handle.
struct ManagedWorker {
    join_handle: JoinHandle<()>,
}

/// Shared dependencies needed to construct a Worker.
/// Stored in the pool by `spawn_initial` so `ensure_capacity` can
/// build additional workers without re-receiving every argument.
struct WorkerDeps {
    task_queue: Arc<TaskQueue>,
    task_store: Arc<TaskStore>,
    machine_pool: Arc<MachinePool>,
    plugin_manager: Arc<PluginManager>,
    event_tx: mpsc::Sender<WorkerEvent>,
    transport: Option<Arc<ResolvedTransport>>,
    sample_store: Arc<SampleStore>,
    result_store: Arc<ResultStore>,
}

/// Pool of workers for task execution.
pub struct WorkerPool {
    workers: StdMutex<Vec<ManagedWorker>>,
    max_workers: usize,
    min_workers: usize,
    idle_timeout: Duration,
    busy_count: Arc<AtomicUsize>,
    shutting_down: Arc<AtomicBool>,
    deps: StdMutex<Option<WorkerDeps>>,
    token: CancellationToken,
}

impl WorkerPool {
    /// Create a new empty worker pool.
    ///
    /// Shared dependencies are supplied later via `spawn_initial`.
    pub fn new(
        max_workers: usize,
        min_workers: usize,
        idle_timeout: Duration,
        token: CancellationToken,
    ) -> Self {
        Self {
            workers: StdMutex::new(Vec::with_capacity(max_workers)),
            max_workers,
            min_workers,
            idle_timeout,
            busy_count: Arc::new(AtomicUsize::new(0)),
            shutting_down: Arc::new(AtomicBool::new(false)),
            deps: StdMutex::new(None),
            token,
        }
    }

    /// Return the number of workers whose join handle is not finished.
    pub fn active_count(&self) -> usize {
        let workers = self.workers.lock().expect("workers mutex poisoned");
        workers
            .iter()
            .filter(|w| !w.join_handle.is_finished())
            .count()
    }

    /// Get the maximum number of workers.
    pub fn max_workers(&self) -> usize {
        self.max_workers
    }

    /// Spawn the baseline pool.
    ///
    /// Stores the shared dependencies (so `ensure_capacity` can spawn
    /// more later) and creates exactly `min_workers` workers with
    /// `idle_timeout = None` — these are the baseline and never idle out.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_initial(
        &self,
        task_queue: Arc<TaskQueue>,
        task_store: Arc<TaskStore>,
        machine_pool: Arc<MachinePool>,
        plugin_manager: Arc<PluginManager>,
        event_tx: mpsc::Sender<WorkerEvent>,
        transport: Option<Arc<ResolvedTransport>>,
        sample_store: Arc<SampleStore>,
        result_store: Arc<ResultStore>,
    ) {
        let deps = WorkerDeps {
            task_queue,
            task_store,
            machine_pool,
            plugin_manager,
            event_tx,
            transport,
            sample_store,
            result_store,
        };

        for _ in 0..self.min_workers {
            self.spawn_one(&deps, None);
        }

        *self.deps.lock().expect("deps mutex poisoned") = Some(deps);

        debug!(count = self.min_workers, "Worker pool initialized");
    }

    /// Spawn a single worker with the given idle_timeout policy.
    /// The new `ManagedWorker` is pushed onto `self.workers`.
    fn spawn_one(&self, deps: &WorkerDeps, idle_timeout: Option<Duration>) {
        let child_token = self.token.child_token();

        let worker = Worker::new(
            Arc::clone(&deps.task_queue),
            Arc::clone(&deps.task_store),
            Arc::clone(&deps.machine_pool),
            Arc::clone(&deps.plugin_manager),
            deps.event_tx.clone(),
            deps.transport.clone(),
            Arc::clone(&deps.sample_store),
            Arc::clone(&deps.result_store),
            Arc::clone(&self.busy_count),
            idle_timeout,
        );

        let worker_id = worker.id().clone();
        debug!(
            worker_id = %worker_id,
            idle_timeout_ms = ?idle_timeout.map(|d| d.as_millis()),
            "Spawning worker"
        );

        let join_handle = tokio::spawn(worker.run(child_token));

        let mut workers = self.workers.lock().expect("workers mutex poisoned");
        workers.push(ManagedWorker { join_handle });
    }

    /// Spawn an additional worker if all live workers are currently busy
    /// and we're below `max_workers`. No-op during shutdown.
    ///
    /// Should be called after every task enqueue.
    pub fn ensure_capacity(&self) {
        use std::sync::atomic::Ordering;

        if self.shutting_down.load(Ordering::Acquire) {
            return;
        }

        // Prune finished handles (workers that idle-exited).
        {
            let mut workers = self.workers.lock().expect("workers mutex poisoned");
            workers.retain(|w| !w.join_handle.is_finished());
        }

        let busy = self.busy_count.load(Ordering::Acquire);
        let active = {
            let workers = self.workers.lock().expect("workers mutex poisoned");
            workers.len()
        };

        if !should_spawn(busy, active, self.max_workers) {
            return;
        }

        let deps_guard = self.deps.lock().expect("deps mutex poisoned");
        let Some(ref deps) = *deps_guard else {
            // `spawn_initial` hasn't run yet — nothing we can do.
            return;
        };

        info!(
            busy,
            active,
            max = self.max_workers,
            "Spawning additional worker (backpressure)"
        );
        self.spawn_one(deps, Some(self.idle_timeout));
    }

    /// Shutdown all workers gracefully.
    pub async fn shutdown(&self) {
        use std::sync::atomic::Ordering;
        info!("Shutting down worker pool");

        self.shutting_down.store(true, Ordering::Release);

        self.token.cancel();

        // Take ownership of managed workers so we can await join handles
        // without holding the mutex (tokio awaits can't cross a std mutex).
        let taken: Vec<ManagedWorker> = {
            let mut workers = self.workers.lock().expect("workers mutex poisoned");
            std::mem::take(&mut *workers)
        };

        for managed in taken {
            if let Err(e) = managed.join_handle.await {
                error!(error = %e, "Worker task panicked during shutdown");
            }
        }

        info!("Worker pool shut down");
    }
}

#[cfg(test)]
mod tests {
    use super::should_spawn;

    #[test]
    fn at_capacity_ceiling_does_not_spawn() {
        assert!(!should_spawn(4, 4, 4));
    }

    #[test]
    fn idle_workers_do_not_spawn() {
        assert!(!should_spawn(0, 2, 4));
        assert!(!should_spawn(1, 2, 4));
    }

    #[test]
    fn all_busy_below_max_spawns() {
        assert!(should_spawn(1, 1, 4));
        assert!(should_spawn(3, 3, 4));
    }

    #[test]
    fn busy_can_exceed_active_momentarily_still_spawns() {
        // busy temporarily > active during race windows — still want to spawn
        assert!(should_spawn(3, 2, 4));
    }

    #[test]
    fn max_zero_never_spawns() {
        assert!(!should_spawn(0, 0, 0));
        assert!(!should_spawn(1, 0, 0));
    }
}
