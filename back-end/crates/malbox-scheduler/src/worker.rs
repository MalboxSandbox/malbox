//! Worker implementation.
//!
//! Each worker is an independent tokio task that competes for tasks
//! from a shared TaskQueue, allocates machines, drives task state
//! transitions, and emits IPC events.

pub mod config;
pub mod event;
pub mod execution;
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
use malbox_database::repositories::samples::fetch_sample_by_id;
use malbox_database::repositories::tasks::TaskState;
use malbox_machinery::{
    Machine as RuntimeMachine, MachineEndpoint, MachineId, MachineState, Platform,
};
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_internal::transport::daemon::GrpcClient;
use malbox_plugin_internal::transport::messages::events::{
    Event, Payload, TaskEvent, TaskEventPayload,
};
use malbox_plugin_internal::transport::traits::TransportEmitter;
use malbox_resources::{MachinePool, ResolvedTransport};
use malbox_utils::SampleStore;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Outcome of a single `execute_task` call.
#[derive(Debug)]
enum TaskOutcome {
    /// Task ran to completion (plugins executed, machine released).
    Completed,
    /// No machine was available — task was re-enqueued for a later attempt.
    Requeued,
}

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
    machine_pool: Arc<MachinePool>,
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
        machine_pool: Arc<MachinePool>,
        plugin_manager: Arc<PluginManager>,
        event_tx: mpsc::Sender<WorkerEvent>,
        transport: Option<Arc<ResolvedTransport>>,
        sample_store: Arc<SampleStore>,
    ) -> Self {
        Self {
            id: WorkerId::new(),
            task_queue,
            task_store,
            machine_pool,
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
            let result = tokio::time::timeout(timeout_duration, self.execute_task(task_id)).await;

            let duration = start.elapsed();

            match result {
                Ok(Ok(TaskOutcome::Completed)) => {
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
                Ok(Ok(TaskOutcome::Requeued)) => {
                    // Task was re-enqueued because no machine was available.
                    // Not a completion — just continue the loop.
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
    ///
    /// Returns `Ok(TaskOutcome::Completed)` when the task ran to completion,
    /// or `Ok(TaskOutcome::Requeued)` when no machine was available and the
    /// task was put back on the queue.
    async fn execute_task(&self, task_id: i32) -> Result<TaskOutcome> {
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

        let db_machine = match self.machine_pool.acquire(task.platform, task_id).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                // No machine available — re-enqueue and wait
                info!(task_id, "No machine available, re-enqueueing task");
                self.task_store
                    .update_task_state(task_id, TaskState::Pending)
                    .await?;
                self.task_queue.enqueue(task_id, task.priority).await;
                tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    self.machine_pool.machine_available.notified(),
                )
                .await
                .ok();
                return Ok(TaskOutcome::Requeued);
            }
            Err(e) => {
                error!(task_id, error = %e, "Machine acquisition failed");
                self.task_store
                    .update_task_state(task_id, TaskState::Failed)
                    .await?;
                self.emit_task_event(TaskEvent::TaskFailed, task_id);
                return Err(SchedulerError::Resource(e));
            }
        };
        let machine_id = db_machine.id.expect("acquired machine has id");
        info!(task_id, machine_id, "Machine acquired");

        // Resolve the target platform from the DB record — used both for
        // building the runtime Machine and for sample execution dispatch.
        let platform = match db_machine.platform {
            malbox_database::repositories::machinery::MachinePlatform::Windows => Platform::Windows,
            malbox_database::repositories::machinery::MachinePlatform::Linux => Platform::Linux,
        };

        // Build a lightweight runtime Machine from the DB record.
        let runtime_machine = {
            let mut m = RuntimeMachine::new(MachineId(
                db_machine.provider_id.clone().unwrap_or_default(),
            ));
            m.set_state(MachineState::Running);
            if let Some(ref ip) = db_machine.ip {
                if let Ok(addr) = ip.parse() {
                    m.set_endpoint(Some(MachineEndpoint {
                        address: addr,
                        id: db_machine.provider_id.clone().unwrap_or_default(),
                        platform,
                    }));
                }
            }
            m
        };

        // --- Running ---
        self.task_store
            .update_task_state(task_id, TaskState::Running)
            .await?;

        // Start the VM and wait for the guest plugin port to become reachable.
        // After a snapshot revert the VM is stopped, so we must boot it first.
        if let Err(e) = self.machine_pool.start_and_wait(&db_machine).await {
            warn!(task_id, machine_id, error = %e, "Failed to start VM, proceeding anyway");
        }

        // Guest access phase: transfer sample to guest.
        // Only activated when a transport is configured and task has a sample.
        if let (Some(transport), Some(sample_id)) = (&self.transport, task.sample_id) {
            let sample = fetch_sample_by_id(self.task_store.pool(), sample_id)
                .await
                .map_err(|e| {
                    SchedulerError::Internal(format!("Failed to fetch sample {}: {}", sample_id, e))
                })?
                .ok_or_else(|| {
                    SchedulerError::Internal(format!("Sample {} not found in database", sample_id))
                })?;

            let host_path = self.sample_store.path(&sample.sha256).map_err(|e| {
                SchedulerError::Internal(format!("Failed to resolve sample path: {}", e))
            })?;

            info!(task_id, sample_id, path = %host_path.display(), "Pushing sample to guest");

            let result = match transport.as_ref() {
                ResolvedTransport::Native {
                    guest_access,
                    transport_name,
                    config,
                } => {
                    let session = guest_access
                        .open_session(transport_name, &runtime_machine, config)
                        .await
                        .map_err(|e| {
                            SchedulerError::Internal(format!(
                                "Failed to open native guest session: {}",
                                e
                            ))
                        })?;
                    let r = session.push_file(&host_path, &task.target).await;
                    if let Err(e) = session.close().await {
                        warn!(task_id, error = %e, "Failed to close guest session cleanly");
                    }
                    r.map_err(|e| {
                        SchedulerError::Internal(format!("Failed to push sample to guest: {}", e))
                    })
                }
                ResolvedTransport::Grpc { .. } => {
                    // Use the acquired machine's IP and the base guest plugin
                    // port (first plugin's port from sequential assignment).
                    let machine_addr = format!(
                        "http://{}:{}",
                        db_machine.ip.as_deref().ok_or_else(|| {
                            SchedulerError::Internal("Machine has no IP for gRPC transport".into())
                        })?,
                        50051u16, // base port — always the first plugin
                    );
                    let mut client =
                        GrpcClient::connect(machine_addr.as_str())
                            .await
                            .map_err(|e| {
                                SchedulerError::Internal(format!(
                                    "Failed to connect gRPC transport: {}",
                                    e
                                ))
                            })?;
                    let data = tokio::fs::read(&host_path).await.map_err(|e| {
                        SchedulerError::Internal(format!("Failed to read sample file: {}", e))
                    })?;
                    let resp = client.push_file(&task.target, data).await.map_err(|e| {
                        SchedulerError::Internal(format!("Failed to push sample to guest: {}", e))
                    })?;
                    if !resp.success {
                        return Err(SchedulerError::Internal(format!(
                            "Guest push_file failed: {}",
                            resp.error_message
                        )));
                    }
                    Ok(())
                }
            };

            if let Err(e) = result {
                error!(task_id, error = %e, "Failed to push sample to guest");
                self.task_store
                    .update_task_state(task_id, TaskState::Failed)
                    .await?;
                self.emit_task_event(TaskEvent::TaskFailed, task_id);

                if let Err(rel_err) = self.machine_pool.release(machine_id).await {
                    error!(task_id, machine_id, error = %rel_err, "Failed to release machine after guest session failure");
                }
                return Err(e);
            }

            info!(
                task_id,
                dest = task.target.as_str(),
                "Sample transferred to guest"
            );
        }

        // --- Sample execution phase ---
        // Execute the sample on the guest OS before running analysis plugins.
        // Uses the first guest plugin's ExecuteCommand RPC to spawn the process
        // in the background so plugins can observe it while it runs.
        {
            let ip = db_machine.ip.as_deref().ok_or_else(|| {
                SchedulerError::Internal("Machine has no IP for sample execution".into())
            })?;
            let exec_addr = format!("http://{}:{}", ip, 50051u16);
            let mut client = GrpcClient::connect(&exec_addr).await.map_err(|e| {
                SchedulerError::Internal(format!("Failed to connect for sample execution: {}", e))
            })?;
            let params = execution::resolve_exec_params(&task.target, platform);
            client
                .execute_command(
                    &params.command,
                    &params.args,
                    params.cwd.as_deref(),
                    &params.env,
                    None,
                    params.background,
                )
                .await
                .map_err(|e| {
                    SchedulerError::Internal(format!("Failed to execute sample on guest: {}", e))
                })?;

            info!(
                task_id,
                command = params.command.as_str(),
                target = task.target.as_str(),
                "Sample execution started on guest"
            );
        }

        // --- Plugin execution phase ---
        // Fetch guest plugins deployed on this machine's active snapshot.
        // Only these plugins are registered — not all guest plugins from the
        // host registry. Port is derived from the plugin's index in the list
        // (base_port + index).
        let snapshot_guest_plugins = self
            .machine_pool
            .get_active_snapshot_guest_plugins(machine_id)
            .await
            .unwrap_or_default();

        let snapshot = self.plugin_manager.registry().snapshot();
        let mut registered_guests = Vec::new();
        let base_port: u16 = 50051;

        if let Some(ref ip) = db_machine.ip {
            for (i, plugin_name) in snapshot_guest_plugins.iter().enumerate() {
                let plugin_id = malbox_plugin_internal::registry::types::PluginId::new(plugin_name);
                let port = base_port + i as u16;
                let addr = format!("http://{}:{}", ip, port);

                match self
                    .plugin_manager
                    .register_guest(&plugin_id, addr.clone())
                    .await
                {
                    Ok(()) => {
                        info!(
                            task_id,
                            plugin_id = plugin_name.as_str(),
                            addr = addr.as_str(),
                            "Registered guest plugin"
                        );
                        registered_guests.push(plugin_id);
                    }
                    Err(e) => {
                        warn!(task_id, plugin_id = plugin_name.as_str(), error = %e, "Failed to register guest plugin");
                    }
                }
            }
        }

        // Run plugins for this task. Guest plugins not in the active snapshot
        // are skipped — they weren't deployed on this VM.
        for entry in snapshot.list() {
            let plugin_id = &entry.id;

            if entry.manifest.plugin.plugin_type
                == malbox_plugin_internal::registry::manifest::PluginTypeConfig::Guest
                && !snapshot_guest_plugins
                    .iter()
                    .any(|n| n == plugin_id.as_str())
            {
                info!(
                    task_id,
                    plugin_id = plugin_id.as_str(),
                    "Skipping guest plugin not in active snapshot"
                );
                continue;
            }

            match self.plugin_manager.acquire(plugin_id).await {
                Ok(handle) => {
                    let config = std::collections::HashMap::new(); // TODO: build from task config
                    match handle.execute_task(task_id, &task.target, config).await {
                        Ok(_results) => {
                            info!(
                                task_id,
                                plugin_id = plugin_id.as_str(),
                                "Plugin execution completed"
                            );
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

        // Unregister guest plugins before machine revert destroys the VM state.
        for plugin_id in &registered_guests {
            self.plugin_manager.unregister_guest(plugin_id);
        }

        // --- Stopping ---
        self.task_store
            .update_task_state(task_id, TaskState::Stopping)
            .await?;

        // --- Release machine ---
        if let Err(e) = self.machine_pool.release(machine_id).await {
            error!(task_id, machine_id, error = %e, "Failed to release machine");
        }

        // --- Completed ---
        self.task_store
            .update_task_state(task_id, TaskState::Completed)
            .await?;
        self.emit_task_event(TaskEvent::TaskCompleted, task_id);

        Ok(TaskOutcome::Completed)
    }

    /// Handle task timeout: transition to Failed and emit event.
    ///
    /// Note: the machine assigned to this task (if any) cannot be released here
    /// because `execute_task` was cancelled by the timeout and we have lost the
    /// `machine_id`. The machine will remain in `assigned` status and will be
    /// recovered by `MachinePool::reconcile()` on the next startup.
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
