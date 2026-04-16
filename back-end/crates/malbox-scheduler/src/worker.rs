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
use malbox_database::repositories::task_results::{self, ResultFormat as DbResultFormat};
use malbox_database::repositories::tasks::TaskState;
use malbox_machinery::{
    Machine as RuntimeMachine, MachineEndpoint, MachineId, MachineState, Platform,
};
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_internal::manager::handle::OutputFormat;
use malbox_plugin_internal::transport::daemon::GrpcClient;
use malbox_plugin_internal::transport::messages::events::Event;
use malbox_plugin_internal::transport::traits::TransportEmitter;
use malbox_resources::{MachinePool, ResolvedTransport};
use malbox_utils::{ResultFormat, ResultStore, SampleStore};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Outcome of a single `execute_task` call.
#[derive(Debug)]
enum TaskOutcome {
    /// Task ran to completion — all plugin results are persisted.
    ///
    /// Machine cleanup (snapshot revert) proceeds asynchronously after
    /// this returns.  The machine's own status (`reverting` → `ready` /
    /// `failed`) tracks that independently from the task.
    Completed,
    /// No machine was available — task was re-enqueued for a later attempt.
    Requeued,
    /// The plugin execution phase exceeded the user-requested analysis timeout.
    TimedOut { analysis_secs: u64 },
    /// The task was canceled because the worker received a shutdown signal.
    Cancelled,
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
    result_store: Arc<ResultStore>,
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
        result_store: Arc<ResultStore>,
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
            result_store,
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
    ///
    /// Shutdown is cooperative: the shutdown signal is converted to a
    /// `watch` channel and passed into `execute_task` so that in-flight
    /// tasks can check for cancellation at safe points and clean up
    /// (release machines) before returning.
    pub async fn run(self, shutdown_rx: tokio::sync::oneshot::Receiver<()>) {
        let notifier = self.task_queue.get_notifier();
        info!(worker_id = %self.id, "Worker started");

        // Convert oneshot into a watch channel so execute_task can poll
        // it cooperatively without consuming the receiver.
        let (shutdown_tx, shutdown) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            let _ = shutdown_rx.await;
            let _ = shutdown_tx.send(true);
        });

        loop {
            // Wait for a task to become available or shutdown signal.
            {
                let mut shutdown_rx = shutdown.clone();
                tokio::select! {
                    _ = notifier.notified() => {},
                    _ = shutdown_rx.wait_for(|v| *v) => {
                        info!(worker_id = %self.id, "Worker received shutdown signal");
                        break;
                    }
                }
            }

            // Shutdown may have arrived between the notifier firing and here.
            if *shutdown.borrow() {
                info!(worker_id = %self.id, "Worker received shutdown signal");
                break;
            }

            // Try to dequeue a task (another worker may have grabbed it)
            let task_id = match self.task_queue.dequeue().await {
                Some(id) => id,
                None => continue,
            };

            info!(worker_id = %self.id, task_id, "Worker picked up task");

            let start = std::time::Instant::now();
            let result = self.execute_task(task_id, &shutdown).await;
            let duration = start.elapsed();

            match result {
                Ok(TaskOutcome::Completed) => {
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
                Ok(TaskOutcome::Requeued) => {
                    // Task was re-enqueued because no machine was available.
                    // Not a completion — just continue the loop.
                }
                Ok(TaskOutcome::TimedOut { analysis_secs }) => {
                    warn!(worker_id = %self.id, task_id, analysis_secs, "Task analysis timed out");
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::JobCompleted {
                            worker_id: self.id.clone(),
                            job_result: Err(SchedulerError::Internal(format!(
                                "Task {} analysis timed out after {}s",
                                task_id, analysis_secs
                            ))),
                            duration,
                        })
                        .await;
                }
                Ok(TaskOutcome::Cancelled) => {
                    info!(worker_id = %self.id, task_id, duration = ?duration, "Task cancelled due to shutdown");
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::JobCompleted {
                            worker_id: self.id.clone(),
                            job_result: Ok(crate::task::TaskResult::new(Some(task_id))),
                            duration,
                        })
                        .await;
                    break;
                }
                Err(e) => {
                    error!(worker_id = %self.id, task_id, error = %e, "Task execution failed");
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::WorkerError {
                            worker_id: self.id.clone(),
                            error: crate::error::WorkerError::ExecutionFailed(e.to_string()),
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
    /// `Ok(TaskOutcome::Requeued)` when no machine was available, or
    /// `Ok(TaskOutcome::Cancelled)` when the worker received a shutdown signal.
    ///
    /// Once a machine is acquired, all subsequent code paths are guarded by
    /// an unconditional cleanup block that releases the machine — even if an
    /// intermediate step fails or the task is cancelled.
    async fn execute_task(
        &self,
        task_id: i32,
        shutdown: &tokio::sync::watch::Receiver<bool>,
    ) -> Result<TaskOutcome> {
        // --- Initializing ---
        self.task_store
            .update_task_state(task_id, TaskState::Initializing)
            .await?;
        self.emit_task_event(Event::TaskStarting { task_id });

        // Load full task for spec building
        let task = self.task_store.load_task(task_id).await?;

        // --- Preparing Resources ---
        self.task_store
            .update_task_state(task_id, TaskState::PreparingResources)
            .await?;

        let db_machine = match self
            .machine_pool
            .acquire(task.platform.clone(), task_id)
            .await
        {
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
                self.emit_task_event(Event::TaskFailed { task_id });
                return Err(SchedulerError::Resource(e));
            }
        };
        let machine_id = db_machine.id.expect("acquired machine has id");
        info!(task_id, machine_id, "Machine acquired");

        // --- Post-acquire: all paths guarded by unconditional cleanup ---
        let analysis_secs = if task.timeout > 0 {
            task.timeout as u64
        } else {
            300
        };
        let mut registered_guests = Vec::new();

        let work_result = self
            .run_task_on_machine(
                task_id,
                &task,
                &db_machine,
                machine_id,
                analysis_secs,
                &mut registered_guests,
                shutdown,
            )
            .await;

        // --- Unconditional cleanup (runs for success, failure, timeout, and cancellation) ---
        for plugin_id in &registered_guests {
            self.plugin_manager.unregister_guest(plugin_id);
        }

        if let Err(e) = self
            .task_store
            .update_task_state(task_id, TaskState::Stopping)
            .await
        {
            error!(task_id, error = %e, "Failed to update task state to Stopping");
        }

        if let Err(e) = self.machine_pool.release(machine_id).await {
            error!(task_id, machine_id, error = %e, "Failed to release machine");
        }

        // --- Post-cleanup: set final task state based on outcome ---
        match &work_result {
            Ok(TaskOutcome::Completed) => {
                self.task_store
                    .update_task_state(task_id, TaskState::Completed)
                    .await?;
                self.emit_task_event(Event::TaskCompleted { task_id });
            }
            Ok(TaskOutcome::TimedOut { .. }) => {
                if let Err(e) = self
                    .task_store
                    .update_task_state(task_id, TaskState::Failed)
                    .await
                {
                    error!(task_id, error = %e, "Failed to update task state to Failed after timeout");
                }
                self.emit_task_event(Event::TaskFailed { task_id });
            }
            Ok(TaskOutcome::Cancelled) => {
                if let Err(e) = self
                    .task_store
                    .update_task_state(task_id, TaskState::Canceled)
                    .await
                {
                    error!(task_id, error = %e, "Failed to update task state to Canceled");
                }
                self.emit_task_event(Event::TaskCanceled { task_id });
            }
            Err(_) => {
                if let Err(e) = self
                    .task_store
                    .update_task_state(task_id, TaskState::Failed)
                    .await
                {
                    error!(task_id, error = %e, "Failed to update task state to Failed");
                }
                self.emit_task_event(Event::TaskFailed { task_id });
            }
            Ok(TaskOutcome::Requeued) => unreachable!("requeue handled before acquire"),
        }

        work_result
    }

    /// Run the task work after a machine has been acquired.
    ///
    /// This method performs all the VM-interaction, sample transfer, plugin
    /// registration/execution, and result persistence.  It is called from
    /// `execute_task` and the caller guarantees that the machine will be
    /// released regardless of the outcome.
    #[allow(clippy::too_many_arguments)]
    async fn run_task_on_machine(
        &self,
        task_id: i32,
        task: &malbox_database::repositories::tasks::Task,
        db_machine: &malbox_database::repositories::machinery::Machine,
        machine_id: i32,
        analysis_secs: u64,
        registered_guests: &mut Vec<malbox_plugin_internal::registry::types::PluginId>,
        shutdown: &tokio::sync::watch::Receiver<bool>,
    ) -> Result<TaskOutcome> {
        // Check cancellation before heavy work.
        if *shutdown.borrow() {
            return Ok(TaskOutcome::Cancelled);
        }

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
        if let Err(e) = self.machine_pool.start_and_wait(db_machine).await {
            warn!(task_id, machine_id, error = %e, "Failed to start VM, proceeding anyway");
        }

        // Check cancellation after VM boot.
        if *shutdown.borrow() {
            return Ok(TaskOutcome::Cancelled);
        }

        // Guest access phase: transfer sample to guest.
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
                    let machine_addr = format!(
                        "http://{}:{}",
                        db_machine.ip.as_deref().ok_or_else(|| {
                            SchedulerError::Internal("Machine has no IP for gRPC transport".into())
                        })?,
                        50051u16,
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
                return Err(e);
            }

            info!(
                task_id,
                dest = task.target.as_str(),
                "Sample transferred to guest"
            );
        }

        // --- Plugin registration phase ---
        let snapshot_guest_plugins = self
            .machine_pool
            .get_active_snapshot_guest_plugins(machine_id)
            .await
            .unwrap_or_default();

        let snapshot = self.plugin_manager.registry().snapshot();
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

        // --- Sample execution phase ---
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

        // Check cancellation before the long analysis phase.
        if *shutdown.borrow() {
            return Ok(TaskOutcome::Cancelled);
        }

        // --- Plugin execution phase ---
        // The analysis timeout wraps only this phase. Shutdown cancellation
        // is also checked here via select! so the worker can exit promptly.
        let analysis_timeout = Duration::from_secs(analysis_secs);
        let mut shutdown_rx = shutdown.clone();

        let timed_out = tokio::select! {
            result = tokio::time::timeout(analysis_timeout, async {
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
                            let mut config = std::collections::HashMap::new();
                            config.insert(
                                "analysis_timeout".to_string(),
                                task.timeout.to_string(),
                            );
                            match handle.execute_task(task_id, &task.target, config).await {
                                Ok(outputs) => {
                                    info!(
                                        task_id,
                                        plugin_id = plugin_id.as_str(),
                                        result_count = outputs.len(),
                                        "Plugin execution completed"
                                    );

                                    let plugin_name = plugin_id.as_str();
                                    for output in &outputs {
                                        let fs_format = match output.format {
                                            OutputFormat::Json => ResultFormat::Json,
                                            OutputFormat::Bytes => ResultFormat::Bytes,
                                        };
                                        let db_format = match output.format {
                                            OutputFormat::Json => DbResultFormat::Json,
                                            OutputFormat::Bytes => DbResultFormat::Bytes,
                                        };

                                        match self
                                            .result_store
                                            .store(
                                                task_id,
                                                plugin_name,
                                                &output.result_name,
                                                fs_format,
                                                &output.data,
                                            )
                                            .await
                                        {
                                            Ok(rel_path) => {
                                                if let Err(e) = task_results::insert_task_result(
                                                    self.task_store.pool(),
                                                    task_id,
                                                    plugin_name,
                                                    &output.result_name,
                                                    db_format,
                                                    output.data.len() as i64,
                                                    &rel_path,
                                                )
                                                .await
                                                {
                                                    error!(
                                                        task_id,
                                                        plugin_name,
                                                        result_name = output.result_name.as_str(),
                                                        error = %e,
                                                        "Failed to insert task result into DB"
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                error!(
                                                    task_id,
                                                    plugin_name,
                                                    result_name = output.result_name.as_str(),
                                                    error = %e,
                                                    "Failed to store task result to filesystem"
                                                );
                                            }
                                        }
                                    }
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
            }) => result.is_err(),
            _ = shutdown_rx.wait_for(|v| *v) => {
                info!(task_id, "Analysis phase cancelled due to shutdown");
                return Ok(TaskOutcome::Cancelled);
            }
        };

        if timed_out {
            warn!(task_id, analysis_secs, "Analysis phase timed out");
            return Ok(TaskOutcome::TimedOut { analysis_secs });
        }

        Ok(TaskOutcome::Completed)
    }

    /// Emit a task IPC event, logging errors but not propagating them.
    fn emit_task_event(&self, event: Event) {
        if let Err(e) = self.plugin_manager.emitter().emit(event) {
            error!(error = %e, "Failed to emit task event");
        }
    }
}
