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
use crate::task::cancel::TaskCancellationRegistry;
use crate::task::queue::TaskQueue;
use crate::task::store::TaskStore;
use crate::task::types::TaskKind;
use malbox_database::repositories::plugin_reports::{
    self, DbClassification, DbConfidence, InsertPluginReport,
};
use malbox_database::repositories::sample_verdicts::{self, UpsertSampleVerdict};
use malbox_database::repositories::samples::fetch_sample_by_id;
use malbox_database::repositories::task_results::{
    self, ResultFormat as DbResultFormat, ResultRole,
};
use malbox_database::repositories::tasks::TaskState;
use malbox_machinery::{
    Machine as RuntimeMachine, MachineEndpoint, MachineId, MachineState, Platform,
};
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_internal::manager::handle::{OutputFormat, PluginHandle, PluginOutput};
use malbox_plugin_internal::transport::daemon::GrpcClient;
use malbox_plugin_internal::transport::messages::events::Event;
use malbox_plugin_internal::transport::traits::TransportEmitter;
use malbox_resources::{MachinePool, ResolvedTransport};
use malbox_utils::{ResultFormat, ResultStore, SampleStore};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, debug, debug_span, error, info, instrument, warn};
use uuid::Uuid;

fn sdk_classification(c: malbox_plugin_sdk::report::Classification) -> DbClassification {
    match c {
        malbox_plugin_sdk::report::Classification::Clean => DbClassification::Clean,
        malbox_plugin_sdk::report::Classification::Unknown => DbClassification::Unknown,
        malbox_plugin_sdk::report::Classification::Suspicious => DbClassification::Suspicious,
        malbox_plugin_sdk::report::Classification::Malicious => DbClassification::Malicious,
    }
}

fn sdk_confidence(c: malbox_plugin_sdk::report::Confidence) -> DbConfidence {
    match c {
        malbox_plugin_sdk::report::Confidence::Low => DbConfidence::Low,
        malbox_plugin_sdk::report::Confidence::Medium => DbConfidence::Medium,
        malbox_plugin_sdk::report::Confidence::High => DbConfidence::High,
    }
}

/// Extra time the host waits beyond the plugin's own analysis budget before
/// declaring the task timed out.
///
/// The plugin typically uses its full `analysis_timeout` for sleep/monitor
/// then flushes and emits a final marker. If the host's wrapping timeout
/// matches the plugin's budget exactly, both timers expire simultaneously:
/// the host drops the gRPC stream at the exact instant the plugin tries to
/// deliver results, so nothing is ever written. The grace covers wake-up,
/// final flush, and the final `send_task_result`.
const ANALYSIS_TIMEOUT_GRACE_SECS: u64 = 60;

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
    /// One or more plugins failed during execution.
    PluginsFailed,
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

/// Increments a shared busy counter on construction and decrements
/// it on drop. Used by workers to signal "currently executing a task"
/// to the `WorkerPool` so it can decide whether to spawn more workers.
struct BusyGuard {
    counter: Arc<std::sync::atomic::AtomicUsize>,
}

impl BusyGuard {
    fn new(counter: Arc<std::sync::atomic::AtomicUsize>) -> Self {
        counter.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Self { counter }
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.counter
            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
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
    busy_count: Arc<std::sync::atomic::AtomicUsize>,
    idle_timeout: Option<Duration>,
    cancel_registry: Arc<TaskCancellationRegistry>,
}

impl Worker {
    /// Create a new worker with all required shared state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        task_queue: Arc<TaskQueue>,
        task_store: Arc<TaskStore>,
        machine_pool: Arc<MachinePool>,
        plugin_manager: Arc<PluginManager>,
        event_tx: mpsc::Sender<WorkerEvent>,
        transport: Option<Arc<ResolvedTransport>>,
        sample_store: Arc<SampleStore>,
        result_store: Arc<ResultStore>,
        busy_count: Arc<std::sync::atomic::AtomicUsize>,
        idle_timeout: Option<Duration>,
        cancel_registry: Arc<TaskCancellationRegistry>,
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
            busy_count,
            idle_timeout,
            cancel_registry,
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
    #[instrument(skip_all, fields(worker_id = %self.id))]
    pub async fn run(self, token: CancellationToken) {
        let notifier = self.task_queue.get_notifier();
        debug!(worker_id = %self.id, "Worker started");

        loop {
            // Wait for task, shutdown, or idle-timeout.
            {
                match self.idle_timeout {
                    None => {
                        tokio::select! {
                            _ = notifier.notified() => {}
                            _ = token.cancelled() => {
                                debug!(worker_id = %self.id, "Worker received shutdown signal");
                                break;
                            }
                        }
                    }
                    Some(timeout) => {
                        tokio::select! {
                            _ = notifier.notified() => {}
                            _ = token.cancelled() => {
                                debug!(worker_id = %self.id, "Worker received shutdown signal");
                                break;
                            }
                            _ = tokio::time::sleep(timeout) => {
                                debug!(worker_id = %self.id, "Worker idle-timeout, exiting");
                                break;
                            }
                        }
                    }
                }
            }

            // Shutdown may have arrived between the notifier firing and here.
            if token.is_cancelled() {
                debug!(worker_id = %self.id, "Worker received shutdown signal");
                break;
            }

            // Try to dequeue a task (another worker may have grabbed it)
            let task_id = match self.task_queue.dequeue().await {
                Some(id) => id,
                None => continue,
            };

            // Skip tasks that were cancelled while queued.
            if let Ok(Some(task)) =
                malbox_database::repositories::tasks::fetch_task(self.task_store.pool(), task_id)
                    .await
                && task.status == TaskState::Canceled
            {
                debug!(worker_id = %self.id, task_id, "Skipping cancelled task");
                continue;
            }

            info!(worker_id = %self.id, task_id, "Worker picked up task");

            let task_token = token.child_token();
            self.cancel_registry
                .register(task_id, task_token.clone())
                .await;

            let start = std::time::Instant::now();
            let result = self.execute_task(task_id, &task_token).await;

            self.cancel_registry.remove(task_id).await;
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
                Ok(TaskOutcome::PluginsFailed) => {
                    error!(worker_id = %self.id, task_id, duration = ?duration, "Task failed due to plugin errors");
                    let _ = self
                        .event_tx
                        .send(WorkerEvent::JobCompleted {
                            worker_id: self.id.clone(),
                            job_result: Err(SchedulerError::Internal(format!(
                                "Task {} failed: one or more plugins encountered errors",
                                task_id
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
            // `_busy` dropped at end of each loop iteration -> fetch_sub happens here.
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
    #[instrument(skip_all, fields(task_id, worker_id = %self.id), err)]
    async fn execute_task(&self, task_id: i32, token: &CancellationToken) -> Result<TaskOutcome> {
        // --- Initializing ---
        self.task_store
            .update_task_state(task_id, TaskState::Initializing)
            .await?;
        self.emit_task_event(Event::TaskStarting { task_id });

        // Load full task for spec building
        let task = self.task_store.load_task(task_id).await?;

        // --- Host-only fast path ---
        let task_kind = TaskKind::classify(&task);
        if matches!(task_kind, TaskKind::HostOnly) {
            info!(task_id, "Host-only task detected, skipping VM acquisition");
            let _busy = BusyGuard::new(Arc::clone(&self.busy_count));

            let work_result = self.execute_host_only_task(task_id, &task, token).await;

            // Stopping state
            if let Err(e) = self
                .task_store
                .update_task_state(task_id, TaskState::Stopping)
                .await
            {
                error!(task_id, error = %e, "Failed to update task state to Stopping");
            }

            // Recompute sample verdict after plugins ran (host-only path).
            if matches!(
                work_result,
                Ok(TaskOutcome::Completed)
                    | Ok(TaskOutcome::PluginsFailed)
                    | Ok(TaskOutcome::TimedOut { .. })
            ) {
                self.recompute_sample_verdict(&task).await;
            }

            // Final state based on outcome (same pattern as the VM path)
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
                Ok(TaskOutcome::PluginsFailed) => {
                    if let Err(e) = self
                        .task_store
                        .update_task_state(task_id, TaskState::Failed)
                        .await
                    {
                        error!(task_id, error = %e, "Failed to update task state to Failed after plugin errors");
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
                Ok(TaskOutcome::Requeued) => unreachable!("host-only tasks don't requeue"),
            }

            return work_result;
        }

        // --- Preparing Resources ---
        self.task_store
            .update_task_state(task_id, TaskState::PreparingResources)
            .await?;

        // TaskKind::HostOnly returned above, so this must be VmBased.
        let TaskKind::VmBased { platform, .. } = task_kind else {
            unreachable!("TaskKind::HostOnly handled above");
        };

        let db_machine = match self.machine_pool.acquire(platform, task_id).await {
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

        // Mark worker as busy only now that it holds a machine. Workers that
        // re-enqueued waiting for capacity do not count toward backpressure,
        // so `ensure_capacity` does not spawn redundant workers that would
        // just requeue themselves.
        let _busy = BusyGuard::new(Arc::clone(&self.busy_count));

        // --- Post-acquire: all paths guarded by unconditional cleanup ---
        let plugin_timeout_secs = if task.timeout > 0 {
            task.timeout as u64
        } else {
            300
        };
        let analysis_secs = plugin_timeout_secs.saturating_add(ANALYSIS_TIMEOUT_GRACE_SECS);
        let mut guest_handles: std::collections::HashMap<
            malbox_plugin_internal::registry::types::PluginId,
            PluginHandle,
        > = std::collections::HashMap::new();

        let work_result = self
            .run_task_on_machine(
                task_id,
                &task,
                &db_machine,
                machine_id,
                analysis_secs,
                &mut guest_handles,
                token,
            )
            .await;

        // --- Unconditional cleanup (runs for success, failure, timeout, and cancellation) ---
        for (_, handle) in guest_handles {
            handle.release().await;
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

        // Recompute sample verdict after plugins ran (VM path).
        if matches!(
            work_result,
            Ok(TaskOutcome::Completed)
                | Ok(TaskOutcome::PluginsFailed)
                | Ok(TaskOutcome::TimedOut { .. })
        ) {
            self.recompute_sample_verdict(&task).await;
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
            Ok(TaskOutcome::PluginsFailed) => {
                if let Err(e) = self
                    .task_store
                    .update_task_state(task_id, TaskState::Failed)
                    .await
                {
                    error!(task_id, error = %e, "Failed to update task state to Failed after plugin errors");
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
    #[instrument(skip_all, fields(task_id, machine_id, worker_id = %self.id), err)]
    #[allow(clippy::too_many_arguments)]
    async fn run_task_on_machine(
        &self,
        task_id: i32,
        task: &malbox_database::repositories::tasks::Task,
        db_machine: &malbox_database::repositories::machinery::Machine,
        machine_id: i32,
        analysis_secs: u64,
        guest_handles: &mut std::collections::HashMap<
            malbox_plugin_internal::registry::types::PluginId,
            PluginHandle,
        >,
        token: &CancellationToken,
    ) -> Result<TaskOutcome> {
        // Check cancellation before heavy work.
        if token.is_cancelled() {
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
            if let Some(ref ip) = db_machine.ip
                && let Ok(addr) = ip.parse()
            {
                m.set_endpoint(Some(MachineEndpoint {
                    address: addr,
                    id: db_machine.provider_id.clone().unwrap_or_default(),
                    platform,
                }));
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
        if token.is_cancelled() {
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

        let base_port: u16 = 50051;

        if let Some(ref ip) = db_machine.ip {
            for (i, plugin_name) in snapshot_guest_plugins.iter().enumerate() {
                async {
                    let plugin_id =
                        malbox_plugin_internal::registry::types::PluginId::new(plugin_name);
                    let port = base_port + i as u16;
                    let addr = format!("http://{}:{}", ip, port);

                    match self
                        .plugin_manager
                        .register_guest(&plugin_id, addr.clone())
                        .await
                    {
                        Ok(handle) => {
                            info!(
                                task_id,
                                plugin = plugin_name.as_str(),
                                addr = addr.as_str(),
                                "Registered guest plugin"
                            );
                            guest_handles.insert(plugin_id, handle);
                        }
                        Err(e) => {
                            warn!(task_id, plugin = plugin_name.as_str(), error = %e, "Failed to register guest plugin");
                        }
                    }
                }
                .instrument(debug_span!("plugin.register", plugin = %plugin_name))
                .await;
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
        if token.is_cancelled() {
            return Ok(TaskOutcome::Cancelled);
        }

        // --- Plugin execution phase ---
        let handles = self.acquire_all_plugins(guest_handles).await;

        let outcome = self
            .run_plugins(task_id, &task.target, analysis_secs, handles, token)
            .await;

        Ok(outcome)
    }

    /// Persist a set of plugin outputs to the filesystem and database.
    async fn persist_plugin_outputs(
        &self,
        task_id: i32,
        plugin_name: &str,
        outputs: &[PluginOutput],
    ) {
        for output in outputs {
            let fs_format = match output.format {
                OutputFormat::Json => ResultFormat::Json,
                OutputFormat::Bytes => ResultFormat::Bytes,
            };
            let db_format = match output.format {
                OutputFormat::Json => DbResultFormat::Json,
                OutputFormat::Bytes => DbResultFormat::Bytes,
            };
            let db_role = if matches!(output.format, OutputFormat::Json)
                && output.result_name == malbox_plugin_transport::REPORT_RESULT_NAME
            {
                ResultRole::Report
            } else {
                ResultRole::Artifact
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
                        &task_results::InsertTaskResult {
                            task_id,
                            plugin_name,
                            result_name: &output.result_name,
                            format: db_format,
                            role: db_role,
                            size_bytes: output.data.len() as i64,
                            file_path: &rel_path,
                        },
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

                    if matches!(db_role, ResultRole::Report)
                        && matches!(db_format, DbResultFormat::Json)
                    {
                        self.extract_plugin_report(task_id, plugin_name, outputs, &output.data)
                            .await;
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

    async fn extract_plugin_report(
        &self,
        task_id: i32,
        plugin_name: &str,
        outputs: &[PluginOutput],
        report_data: &[u8],
    ) {
        let report = match serde_json::from_slice::<malbox_plugin_sdk::report::Report>(report_data)
        {
            Ok(r) => r,
            Err(e) => {
                warn!(task_id, plugin_name, error = %e, "Failed to parse report for metadata extraction");
                return;
            }
        };

        let artifact_count = outputs
            .iter()
            .filter(|o| o.result_name != malbox_plugin_transport::REPORT_RESULT_NAME)
            .count() as i32;

        let labels: Vec<String> = report
            .verdict
            .as_ref()
            .map(|v| v.labels.clone())
            .unwrap_or_default();

        let indicators_json = serde_json::to_value(&report.indicators).unwrap_or_default();
        let ttps_json = serde_json::to_value(&report.ttps).unwrap_or_default();

        let params = InsertPluginReport {
            task_id,
            plugin_name,
            display_name: report.plugin.display_name.as_deref(),
            plugin_version: &report.plugin.version,
            classification: report
                .verdict
                .as_ref()
                .map(|v| sdk_classification(v.classification)),
            score: report
                .verdict
                .as_ref()
                .and_then(|v| v.score.map(|s| s as i16)),
            confidence: report
                .verdict
                .as_ref()
                .and_then(|v| v.confidence.map(sdk_confidence)),
            labels: &labels,
            indicators: &indicators_json,
            ttps: &ttps_json,
            summary: report.summary.as_deref(),
            section_count: report.sections.len() as i32,
            artifact_count,
        };

        if let Err(e) = plugin_reports::insert_plugin_report(self.task_store.pool(), &params).await
        {
            error!(task_id, plugin_name, error = %e, "Failed to insert plugin report metadata");
        }
    }

    async fn recompute_sample_verdict(&self, task: &malbox_database::repositories::tasks::Task) {
        let Some(sample_id) = task.sample_id else {
            return;
        };
        let pool = self.task_store.pool();

        let reports = match plugin_reports::fetch_latest_plugin_reports_for_sample(pool, sample_id)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!(sample_id, error = %e, "Failed to fetch latest plugin reports for verdict");
                return;
            }
        };

        let task_count =
            match plugin_reports::count_reported_tasks_for_sample(pool, sample_id).await {
                Ok(n) => n as i32,
                Err(e) => {
                    error!(sample_id, error = %e, "Failed to count reported tasks for verdict");
                    return;
                }
            };

        let mut worst: Option<DbClassification> = None;
        let mut max_score: Option<i16> = None;
        let mut plugin_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut seen_iocs: std::collections::BTreeSet<(String, String)> =
            std::collections::BTreeSet::new();
        let mut seen_ttps: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

        for r in &reports {
            plugin_set.insert(r.plugin_name.clone());

            if let Some(c) = r.classification {
                worst = Some(match worst {
                    Some(w) if w.severity() >= c.severity() => w,
                    _ => c,
                });
            }
            if let Some(s) = r.score {
                max_score = Some(max_score.map_or(s, |m| m.max(s)));
            }

            if let Some(arr) = r.indicators.as_array() {
                for ind in arr {
                    if let (Some(k), Some(v)) = (ind["kind"].as_str(), ind["value"].as_str()) {
                        seen_iocs.insert((k.to_string(), v.to_string()));
                    }
                }
            }
            if let Some(arr) = r.ttps.as_array() {
                for ttp in arr {
                    if let Some(id) = ttp["id"].as_str() {
                        seen_ttps.insert(id.to_string());
                    }
                }
            }
        }

        let mut names: Vec<String> = plugin_set.into_iter().collect();
        names.sort();

        let params = UpsertSampleVerdict {
            sample_id,
            classification: worst,
            score: max_score,
            indicator_count: seen_iocs.len() as i32,
            ttp_count: seen_ttps.len() as i32,
            plugin_names: &names,
            task_count,
            last_task_id: task.id,
        };

        if let Err(e) = sample_verdicts::upsert_sample_verdict(pool, &params).await {
            error!(sample_id, error = %e, "Failed to upsert sample verdict");
        }
    }

    /// Run a set of plugins sequentially within an analysis timeout, persisting
    /// their outputs. Returns the overall task outcome.
    async fn run_plugins(
        &self,
        task_id: i32,
        sample_path: &str,
        analysis_secs: u64,
        handles: Vec<(
            malbox_plugin_internal::registry::types::PluginId,
            PluginHandle,
        )>,
        token: &CancellationToken,
    ) -> TaskOutcome {
        let analysis_timeout = Duration::from_secs(analysis_secs);
        let mut cancelled = false;
        let mut has_plugin_failure = false;
        let mut iter = handles.into_iter();

        let timed_out = tokio::time::timeout(analysis_timeout, async {
            for (plugin_id, handle) in iter.by_ref() {
                if token.is_cancelled() {
                    cancelled = true;
                    handle.release().await;
                    break;
                }

                let plugin_ok = async {
                    let mut config = std::collections::HashMap::new();
                    let plugin_budget =
                        analysis_secs.saturating_sub(ANALYSIS_TIMEOUT_GRACE_SECS);
                    config.insert(
                        "analysis_timeout".to_string(),
                        plugin_budget.to_string(),
                    );

                    let ok =
                        match handle.execute_task(task_id, sample_path, config).await {
                            Ok(outputs) => {
                                info!(
                                    task_id,
                                    plugin = plugin_id.as_str(),
                                    result_count = outputs.len(),
                                    "Plugin execution completed"
                                );
                                self.persist_plugin_outputs(
                                    task_id,
                                    plugin_id.as_str(),
                                    &outputs,
                                )
                                .await;
                                true
                            }
                            Err(e) => {
                                error!(task_id, plugin = plugin_id.as_str(), error = %e, "Plugin execution failed");
                                false
                            }
                        };
                    handle.release().await;
                    ok
                }
                .instrument(debug_span!("plugin.execute", plugin = %plugin_id))
                .await;

                if !plugin_ok {
                    has_plugin_failure = true;
                }
            }
        })
        .await
        .is_err();

        // Release any handles that weren't reached (early break or timeout).
        for (_, handle) in iter {
            handle.release().await;
        }

        if cancelled {
            TaskOutcome::Cancelled
        } else if timed_out {
            TaskOutcome::TimedOut { analysis_secs }
        } else if has_plugin_failure {
            TaskOutcome::PluginsFailed
        } else {
            TaskOutcome::Completed
        }
    }

    /// Acquire plugin handles for a specific list of plugins (host-only path).
    ///
    /// Guest plugin handles are taken from `guest_handles` if present; host
    /// plugins are freshly acquired from the plugin manager.
    async fn acquire_task_plugins(
        &self,
        plugin_names: &[String],
        guest_handles: &mut std::collections::HashMap<
            malbox_plugin_internal::registry::types::PluginId,
            PluginHandle,
        >,
    ) -> Vec<(
        malbox_plugin_internal::registry::types::PluginId,
        PluginHandle,
    )> {
        let snapshot = self.plugin_manager.registry().snapshot();
        let mut handles = Vec::new();

        for plugin_name in plugin_names {
            let plugin_id = malbox_plugin_internal::registry::types::PluginId::new(plugin_name);

            let entry = match snapshot.get(&plugin_id) {
                Some(e) => e,
                None => {
                    warn!(
                        plugin = plugin_name.as_str(),
                        "Plugin not found in registry, skipping"
                    );
                    continue;
                }
            };

            let handle = match entry.manifest.plugin.plugin_type {
                malbox_plugin_internal::registry::manifest::PluginTypeConfig::Guest => {
                    guest_handles.remove(&plugin_id)
                }
                malbox_plugin_internal::registry::manifest::PluginTypeConfig::Host => {
                    match self.plugin_manager.acquire(&plugin_id).await {
                        Ok(h) => Some(h),
                        Err(e) => {
                            warn!(plugin = plugin_name.as_str(), error = %e, "Failed to acquire plugin");
                            None
                        }
                    }
                }
            };

            if let Some(h) = handle {
                handles.push((plugin_id, h));
            }
        }

        handles
    }

    /// Acquire plugin handles for ALL registered plugins (VM path).
    ///
    /// Iterates every plugin in the registry snapshot. Guest handles are taken
    /// from `guest_handles`; host plugins are freshly acquired.
    async fn acquire_all_plugins(
        &self,
        guest_handles: &mut std::collections::HashMap<
            malbox_plugin_internal::registry::types::PluginId,
            PluginHandle,
        >,
    ) -> Vec<(
        malbox_plugin_internal::registry::types::PluginId,
        PluginHandle,
    )> {
        let snapshot = self.plugin_manager.registry().snapshot();
        let mut handles = Vec::new();

        for entry in snapshot.list() {
            let plugin_id = &entry.id;

            let handle = match entry.manifest.plugin.plugin_type {
                malbox_plugin_internal::registry::manifest::PluginTypeConfig::Guest => {
                    guest_handles.remove(plugin_id)
                }
                malbox_plugin_internal::registry::manifest::PluginTypeConfig::Host => {
                    match self.plugin_manager.acquire(plugin_id).await {
                        Ok(h) => Some(h),
                        Err(e) => {
                            warn!(plugin = plugin_id.as_str(), error = %e, "Failed to acquire plugin");
                            None
                        }
                    }
                }
            };

            if let Some(h) = handle {
                handles.push((plugin_id.clone(), h));
            }
        }

        handles
    }

    /// Execute a host-only task (no VM, no guest plugins).
    ///
    /// Runs only plugins listed in `task.plugins`, skipping VM interaction,
    /// sample transfer, and guest plugin registration.
    #[instrument(skip_all, fields(task_id, worker_id = %self.id), err)]
    async fn execute_host_only_task(
        &self,
        task_id: i32,
        task: &malbox_database::repositories::tasks::Task,
        token: &CancellationToken,
    ) -> Result<TaskOutcome> {
        if token.is_cancelled() {
            return Ok(TaskOutcome::Cancelled);
        }

        self.task_store
            .update_task_state(task_id, TaskState::Running)
            .await?;

        let sample_path = if let Some(sample_id) = task.sample_id {
            let sample = fetch_sample_by_id(self.task_store.pool(), sample_id)
                .await
                .map_err(|e| {
                    SchedulerError::Internal(format!("Failed to fetch sample {}: {}", sample_id, e))
                })?
                .ok_or_else(|| {
                    SchedulerError::Internal(format!("Sample {} not found in database", sample_id))
                })?;
            let path = self.sample_store.path(&sample.sha256).map_err(|e| {
                SchedulerError::Internal(format!("Failed to resolve sample path: {}", e))
            })?;
            path.to_string_lossy().to_string()
        } else {
            task.target.clone()
        };

        let plugin_timeout_secs = if task.timeout > 0 {
            task.timeout as u64
        } else {
            300
        };
        let analysis_secs = plugin_timeout_secs.saturating_add(ANALYSIS_TIMEOUT_GRACE_SECS);

        let mut empty_guests = std::collections::HashMap::new();
        let handles = self
            .acquire_task_plugins(&task.plugins, &mut empty_guests)
            .await;

        Ok(self
            .run_plugins(task_id, &sample_path, analysis_secs, handles, token)
            .await)
    }

    /// Emit a task IPC event, logging errors but not propagating them.
    fn emit_task_event(&self, event: Event) {
        if let Err(e) = self.plugin_manager.emitter().emit(event) {
            error!(error = %e, "Failed to emit task event");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BusyGuard;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn busy_guard_increments_on_new_and_decrements_on_drop() {
        let counter = Arc::new(AtomicUsize::new(0));
        assert_eq!(counter.load(Ordering::Acquire), 0);

        let guard = BusyGuard::new(Arc::clone(&counter));
        assert_eq!(counter.load(Ordering::Acquire), 1);

        drop(guard);
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }

    #[test]
    fn busy_guard_nested_counts_correctly() {
        let counter = Arc::new(AtomicUsize::new(0));
        let g1 = BusyGuard::new(Arc::clone(&counter));
        let g2 = BusyGuard::new(Arc::clone(&counter));
        assert_eq!(counter.load(Ordering::Acquire), 2);
        drop(g1);
        assert_eq!(counter.load(Ordering::Acquire), 1);
        drop(g2);
        assert_eq!(counter.load(Ordering::Acquire), 0);
    }
}
