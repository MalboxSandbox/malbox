//! Worker implementation.
//!
//! Each worker is an independent tokio task that competes for tasks
//! from a shared TaskQueue, allocates machines, drives task state
//! transitions, and emits IPC events.

pub mod config;
pub mod event;
pub mod handle;
pub mod job;
pub mod pool;

pub use config::WorkerConfig;
pub use event::WorkerEvent;
pub use handle::WorkerHandle;
pub use job::Job;

use crate::error::{Result, SchedulerError};
use crate::task::queue::TaskQueue;
use crate::task::store::TaskStore;
use crate::task::task_to_machine_spec;
use malbox_config::MachineryConfig;
use malbox_database::repositories::samples::fetch_sample_by_id;
use malbox_database::repositories::tasks::TaskState;
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_internal::transport::messages::events::{
    Event, Payload, TaskEvent, TaskEventPayload,
};
use malbox_plugin_internal::transport::traits::TransportEmitter;
use malbox_resources::{MachineryManager, ResolvedTransport};
use malbox_utils::SampleStore;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Unique identifier for a worker instance.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WorkerId(Uuid);

impl WorkerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for WorkerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for WorkerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Show first 8 chars for readable logs
        write!(f, "{}", &self.0.to_string()[..8])
    }
}

/// A worker that executes tasks by competing for them from a shared queue.
pub struct Worker {
    id: WorkerId,
    task_queue: Arc<TaskQueue>,
    task_store: Arc<TaskStore>,
    machinery_manager: Arc<dyn MachineryManager>,
    machinery_config: MachineryConfig,
    plugin_manager: Arc<PluginManager>,
    event_tx: mpsc::Sender<WorkerEvent>,
    transport: Option<Arc<ResolvedTransport>>,
    sample_store: Arc<SampleStore>,
}

impl Worker {
    /// Create a new worker with all required shared state.
    pub fn new(
        task_queue: Arc<TaskQueue>,
        task_store: Arc<TaskStore>,
        machinery_manager: Arc<dyn MachineryManager>,
        machinery_config: MachineryConfig,
        plugin_manager: Arc<PluginManager>,
        event_tx: mpsc::Sender<WorkerEvent>,
        transport: Option<Arc<ResolvedTransport>>,
        sample_store: Arc<SampleStore>,
    ) -> Self {
        Self {
            id: WorkerId::new(),
            task_queue,
            task_store,
            machinery_manager,
            machinery_config,
            plugin_manager,
            event_tx,
            transport,
            sample_store,
        }
    }

    /// Get the worker's ID.
    pub fn id(&self) -> &WorkerId {
        &self.id
    }

    /// Run the worker's main loop.
    ///
    /// The worker waits for tasks on the shared queue's notifier,
    /// dequeues a task (competing with other workers), and executes
    /// the full task lifecycle.
    pub async fn run(self, mut shutdown_rx: tokio::sync::oneshot::Receiver<()>) {
        let notifier = self.task_queue.get_notifier();
        info!(worker_id = %self.id, "Worker started");

        loop {
            // Wait for a task to become available or shutdown signal
            tokio::select! {
                _ = notifier.notified() => {},
                _ = &mut shutdown_rx => {
                    info!(worker_id = %self.id, "Worker received shutdown signal");
                    break;
                }
            }

            // Try to dequeue a task (another worker may have grabbed it)
            let task_id = match self.task_queue.dequeue().await {
                Some(id) => id,
                None => continue,
            };

            info!(worker_id = %self.id, task_id, "Worker picked up task");

            // Load full task from store
            let task = match self.task_store.load_task(task_id).await {
                Ok(t) => t,
                Err(e) => {
                    error!(worker_id = %self.id, task_id, error = %e, "Failed to load task");
                    continue;
                }
            };

            // Determine timeout duration
            let timeout_secs = task.timeout as u64;
            let timeout_duration = if timeout_secs > 0 {
                Duration::from_secs(timeout_secs)
            } else {
                Duration::from_secs(300) // Default 5 min timeout
            };

            // Execute with timeout
            let start = std::time::Instant::now();
            let result =
                tokio::time::timeout(timeout_duration, self.execute_task(task_id)).await;

            let duration = start.elapsed();

            match result {
                Ok(Ok(())) => {
                    info!(worker_id = %self.id, task_id, duration = ?duration, "Task completed successfully");
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::JobCompleted {
                            worker_id: self.id.clone(),
                            job_result: Ok(crate::task::TaskResult::new(Some(task_id))),
                            duration,
                        })
                        .await;
                }
                Ok(Err(e)) => {
                    error!(worker_id = %self.id, task_id, error = %e, "Task execution failed");
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::WorkerError {
                            worker_id: self.id.clone(),
                            error: crate::error::WorkerError::ExecutionFailed(e.to_string()),
                        })
                        .await;
                }
                Err(_) => {
                    warn!(worker_id = %self.id, task_id, timeout = timeout_secs, "Task timed out");
                    if let Err(e) = self.handle_timeout(task_id).await {
                        error!(worker_id = %self.id, task_id, error = %e, "Failed to handle timeout cleanup");
                    }
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::JobCompleted {
                            worker_id: self.id.clone(),
                            job_result: Err(SchedulerError::Internal(format!(
                                "Task {} timed out after {}s",
                                task_id, timeout_secs
                            ))),
                            duration,
                        })
                        .await;
                }
            }
        }

        info!(worker_id = %self.id, "Worker stopped");
    }

    /// Execute the full lifecycle of a single task.
    async fn execute_task(&self, task_id: i32) -> Result<()> {
        // --- Initializing ---
        self.task_store
            .update_task_state(task_id, TaskState::Initializing)
            .await?;
        self.emit_task_event(TaskEvent::TaskStarting, task_id);

        // Load full task for spec building
        let task = self.task_store.load_task(task_id).await?;

        // --- Preparing Resources ---
        self.task_store
            .update_task_state(task_id, TaskState::PreparingResources)
            .await?;

        let spec = task_to_machine_spec(&task, &self.machinery_config);
        let machine = match self.machinery_manager.allocate(&spec).await {
            Ok(m) => m,
            Err(e) => {
                error!(task_id, error = %e, "Machine allocation failed");
                self.task_store
                    .update_task_state(task_id, TaskState::Failed)
                    .await?;
                self.emit_task_event(TaskEvent::TaskFailed, task_id);
                return Err(SchedulerError::Resource(e));
            }
        };

        info!(task_id, machine_id = ?machine.id, "Machine allocated");

        // --- Running ---
        self.task_store
            .update_task_state(task_id, TaskState::Running)
            .await?;

        // Guest access phase: transfer sample, execute, collect results.
        // Only activated when a transport is configured.
        if let Some(ref transport) = self.transport {
            match transport.open_session(&machine).await {
                Ok(session) => {
                    // Push sample file to guest if task has a sample
                    if let Some(sample_id) = task.sample_id {
                        let sample = fetch_sample_by_id(
                            self.task_store.pool(),
                            sample_id,
                        )
                        .await
                        .map_err(|e| SchedulerError::Internal(
                            format!("Failed to fetch sample {}: {}", sample_id, e),
                        ))?
                        .ok_or_else(|| SchedulerError::Internal(
                            format!("Sample {} not found in database", sample_id),
                        ))?;

                        let host_path = self.sample_store.path(&sample.sha256)
                            .map_err(|e| SchedulerError::Internal(
                                format!("Failed to resolve sample path: {}", e),
                            ))?;

                        info!(task_id, sample_id, path = %host_path.display(), "Pushing sample to guest");

                        session.push_file(&host_path, &task.target).await
                            .map_err(|e| SchedulerError::Internal(
                                format!("Failed to push sample to guest: {}", e),
                            ))?;

                        info!(task_id, dest = task.target.as_str(), "Sample transferred to guest");
                    } else {
                        warn!(task_id, "Task has no sample_id, skipping guest file transfer");
                    }

                    if let Err(e) = session.close().await {
                        warn!(task_id, error = %e, "Failed to close guest session cleanly");
                    }
                }
                Err(e) => {
                    error!(task_id, error = %e, "Failed to open guest session");
                    self.task_store
                        .update_task_state(task_id, TaskState::Failed)
                        .await?;
                    self.emit_task_event(TaskEvent::TaskFailed, task_id);

                    // Still release the machine
                    if let Err(rel_err) = self.machinery_manager.release(&machine).await {
                        error!(task_id, error = %rel_err, "Failed to release machine after guest session failure");
                    }
                    return Err(SchedulerError::Internal(format!(
                        "Guest session failed for task {}: {}",
                        task_id, e
                    )));
                }
            }
        }

        // --- Plugin execution phase ---
        // Run all registered plugins for this task via the plugin manager.
        let snapshot = self.plugin_manager.registry().snapshot();
        // TODO: Filter plugins based on task type, scope, manifest config
        for entry in snapshot.list() {
            let plugin_id = &entry.id;

            match self.plugin_manager.acquire(plugin_id).await {
                Ok(handle) => {
                    let config = std::collections::HashMap::new(); // TODO: build from task config
                    match handle.execute_task(task_id, "", config).await {
                        Ok(_results) => {
                            info!(task_id, plugin_id = plugin_id.as_str(), "Plugin execution completed");
                            // TODO: collect results into TaskResult
                        }
                        Err(e) => {
                            error!(task_id, plugin_id = plugin_id.as_str(), error = %e, "Plugin execution failed");
                        }
                    }
                    handle.release().await;
                }
                Err(e) => {
                    warn!(task_id, plugin_id = plugin_id.as_str(), error = %e, "Failed to acquire plugin");
                }
            }
        }

        // --- Stopping ---
        self.task_store
            .update_task_state(task_id, TaskState::Stopping)
            .await?;

        // --- Release machine ---
        if let Err(e) = self.machinery_manager.release(&machine).await {
            error!(task_id, machine_id = ?machine.id, error = %e, "Failed to release machine");
        }

        // --- Completed ---
        self.task_store
            .update_task_state(task_id, TaskState::Completed)
            .await?;
        self.emit_task_event(TaskEvent::TaskCompleted, task_id);

        Ok(())
    }

    /// Handle task timeout: transition to Failed and emit event.
    async fn handle_timeout(&self, task_id: i32) -> Result<()> {
        self.task_store
            .update_task_state(task_id, TaskState::Failed)
            .await?;
        self.emit_task_event(TaskEvent::TaskFailed, task_id);
        Ok(())
    }

    /// Emit a task IPC event, logging errors but not propagating them.
    fn emit_task_event(&self, event: TaskEvent, task_id: i32) {
        if let Err(e) = self.plugin_manager.emitter().emit(
            Event::Task(event),
            Payload::Task(TaskEventPayload { task_id }),
        ) {
            error!(task_id, error = %e, "Failed to emit task event");
        }
    }
}
