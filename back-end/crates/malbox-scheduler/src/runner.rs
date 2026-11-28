//! Job runners: the boundary between the orchestrator and the plugin
//! manager / machine pool. Tests substitute their own implementations.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use malbox_database::PgPool;
use malbox_database::repositories::artifacts::TaskArtifact;
use malbox_database::repositories::jobs::Job;
use malbox_database::repositories::machinery::{Machine as DbMachine, MachinePlatform};
use malbox_database::repositories::snapshots::{self, MachineSnapshot};
use malbox_database::repositories::tasks::Task;
use malbox_machinery::{
    ExecOptions, GuestSession, Machine as RuntimeMachine, MachineEndpoint, MachineId, MachineState,
    Platform,
};
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_internal::manager::error::ManagerError;
use malbox_plugin_internal::manager::handle::{PluginHandle, PluginOutput};
use malbox_plugin_internal::registry::types::PluginId;
use malbox_plugin_internal::transport::daemon::{ExecMode, GrpcClient};
use malbox_plugin_transport::job_config;
use malbox_resources::{AcquireOutcome, MachinePool, MachineRequest, ResolvedTransport};
use thiserror::Error;
use tokio::sync::{Semaphore, oneshot};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use crate::package;
use crate::plan::DetonationRequest;
use crate::routing::AnalyzerSpec;

/// Port of the first guest plugin in a snapshot; the n-th listens on
/// `GUEST_BASE_PORT + n`.
const GUEST_BASE_PORT: u16 = 50051;
/// How long a host runner retries a busy plugin before giving up.
const PLUGIN_ACQUIRE_TIMEOUT: Duration = Duration::from_secs(30);
const PLUGIN_ACQUIRE_RETRY: Duration = Duration::from_millis(500);
/// How long a detonation waits for every guest plugin's READY.
const GUEST_READY_TIMEOUT: Duration = Duration::from_mins(1);
/// Extra stream time beyond the observation window: READY wait, launch,
/// and the 120 s collection grace after the stop request.
const GUEST_STREAM_SLACK_SECS: u64 = 180;

/// Pseudo plugin name of the entry a detonation runner appends when the
/// sample could not be launched. Its error fails the job as
/// non-recoverable while the real plugins' outputs are still ingested.
pub(crate) const LAUNCH_FAILURE_PLUGIN: &str = "malbox.launch";

/// Everything a host runner needs to run one host job.
#[derive(Debug)]
pub struct HostJobRequest {
    /// Task the job belongs to.
    pub task_id: i32,
    /// Claimed job row.
    pub job: Job,
    /// Host plugin that runs the job.
    pub spec: AnalyzerSpec,
    /// Path of the artifact blob to analyse.
    pub sample_path: PathBuf,
    /// Job configuration passed to the plugin.
    pub config: HashMap<String, String>,
}

/// Why a job run failed; the variant decides retry behaviour.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum JobRunError {
    /// The run exceeded its time limit; retried.
    #[error("{0}")]
    Timeout(String),
    /// A transient failure; retried.
    #[error("{0}")]
    Recoverable(String),
    /// A permanent failure; the job fails without retry.
    #[error("{0}")]
    Nonrecoverable(String),
    /// The analysis machine became unusable; retried on another machine.
    #[error("{0}")]
    MachineLost(String),
    /// The run was canceled by the user or shutdown.
    #[error("canceled")]
    Canceled,
}

/// Runs host jobs; the production runner drives plugins via the plugin manager.
#[async_trait]
pub trait HostJobRunner: Send + Sync {
    /// Run one host job until it finishes or `cancel` fires.
    async fn run(
        &self,
        req: HostJobRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<PluginOutput>, JobRunError>;
}

/// Everything a detonation runner needs to run one detonation job.
#[derive(Debug)]
pub struct DetonationJobRequest {
    /// Task the job belongs to.
    pub task: Task,
    /// Claimed job row.
    pub job: Job,
    /// Artifact being detonated.
    pub artifact: TaskArtifact,
    /// Resolved detonation parameters.
    pub request: DetonationRequest,
    /// Path of the artifact blob on the host.
    pub sample_path: PathBuf,
    /// Job configuration shared by every guest plugin.
    pub base_config: HashMap<String, String>,
}

/// One guest plugin's result from a detonation.
#[derive(Debug)]
pub struct PluginOutputs {
    /// Guest plugin name, or [`LAUNCH_FAILURE_PLUGIN`].
    pub plugin: String,
    /// Outputs the plugin produced, or why it failed.
    pub result: Result<Vec<PluginOutput>, JobRunError>,
}

/// A machine reserved for one detonation.
#[derive(Debug, Clone)]
pub struct Lease {
    /// Database id of the reserved machine.
    pub machine_id: i32,
    /// Snapshot the machine is restored to.
    pub snapshot_id: Uuid,
}

/// Outcome of trying to reserve a machine for a detonation.
#[derive(Debug)]
#[non_exhaustive]
pub enum AcquireResult {
    /// A machine was reserved.
    Acquired(Lease),
    /// No matching machine is free right now; try again later.
    Busy,
    /// No configured machine can ever serve the request; the message says why.
    Unserviceable(String),
}

/// Reserves machines and runs detonations; production uses the machine pool.
#[async_trait]
pub trait DetonationRunner: Send + Sync {
    /// Try to reserve a machine matching `request` for `task_id`.
    async fn acquire(&self, request: &DetonationRequest, task_id: i32) -> AcquireResult;
    /// Release a lease whose job could not be claimed.
    async fn abandon(&self, lease: Lease);
    /// Run one detonation session. A launch failure is reported as an extra
    /// [`LAUNCH_FAILURE_PLUGIN`] entry next to the plugins' outputs.
    async fn run(
        &self,
        lease: Lease,
        req: DetonationJobRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<PluginOutputs>, JobRunError>;
}

/// Runs host jobs on plugins owned by the [`PluginManager`].
pub(crate) struct PluginManagerHostRunner {
    pub(crate) manager: Arc<PluginManager>,
}

impl PluginManagerHostRunner {
    async fn kill(&self, handle: PluginHandle) {
        let plugin = handle.plugin_id().clone();
        if let Err(e) = self.manager.kill_handle(handle).await {
            warn!(%plugin, error = %e, "failed to kill plugin");
        }
    }
}

#[async_trait]
impl HostJobRunner for PluginManagerHostRunner {
    async fn run(
        &self,
        req: HostJobRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<PluginOutput>, JobRunError> {
        let id = PluginId::new(&req.spec.name);
        let give_up = Instant::now() + PLUGIN_ACQUIRE_TIMEOUT;
        let handle = loop {
            match self.manager.acquire(&id, req.task_id).await {
                Ok(h) => break h,
                Err(ManagerError::PluginBusy(_)) => {
                    if Instant::now() >= give_up {
                        return Err(JobRunError::Recoverable("plugin not ready".into()));
                    }
                    tokio::select! {
                        () = tokio::time::sleep(PLUGIN_ACQUIRE_RETRY) => {}
                        () = cancel.cancelled() => return Err(JobRunError::Canceled),
                    }
                }
                Err(e @ ManagerError::PluginNotFound(_)) => {
                    return Err(JobRunError::Nonrecoverable(e.to_string()));
                }
                Err(e) => return Err(JobRunError::Recoverable(e.to_string())),
            }
        };

        let sample_path = req.sample_path.to_string_lossy().into_owned();
        let outcome = tokio::select! {
            r = handle.execute_task(req.task_id, &sample_path, req.config, req.spec.timeout, None) => Some(r),
            () = cancel.cancelled() => None,
        };
        match outcome {
            None => {
                self.kill(handle).await;
                Err(JobRunError::Canceled)
            }
            Some(Ok(outputs)) => {
                handle.release().await;
                Ok(outputs)
            }
            Some(Err(e @ ManagerError::Timeout(_))) => {
                self.kill(handle).await;
                Err(JobRunError::Timeout(e.to_string()))
            }
            Some(Err(e @ ManagerError::Crashed(_))) => {
                self.kill(handle).await;
                Err(JobRunError::Recoverable(e.to_string()))
            }
            Some(Err(e @ (ManagerError::Reported(..) | ManagerError::PluginNotFound(_)))) => {
                handle.release().await;
                Err(JobRunError::Nonrecoverable(e.to_string()))
            }
            Some(Err(e)) => {
                handle.release().await;
                Err(JobRunError::Recoverable(e.to_string()))
            }
        }
    }
}

/// Runs detonations on machines from the [`MachinePool`].
pub(crate) struct MachineDetonationRunner {
    pub(crate) db: PgPool,
    pub(crate) pool: Arc<MachinePool>,
    pub(crate) manager: Arc<PluginManager>,
    pub(crate) transport: Option<Arc<ResolvedTransport>>,
    pub(crate) boot: Arc<Semaphore>,
    pub(crate) guest_sample_dir: Option<String>,
}

impl MachineDetonationRunner {
    pub(crate) fn new(
        db: PgPool,
        pool: Arc<MachinePool>,
        manager: Arc<PluginManager>,
        transport: Option<Arc<ResolvedTransport>>,
        max_concurrent_vm_boots: u32,
    ) -> Self {
        let guest_sample_dir = match transport.as_deref() {
            Some(ResolvedTransport::Native { config, .. }) => config
                .get("sample_dir")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            _ => None,
        };
        Self {
            db,
            pool,
            manager,
            transport,
            boot: Arc::new(Semaphore::new(
                usize::try_from(max_concurrent_vm_boots.max(1)).unwrap_or(usize::MAX),
            )),
            guest_sample_dir,
        }
    }
}

/// Releases the machine on every exit path unless disarmed (after
/// `mark_failed` took it out of rotation).
struct ReleaseGuard {
    pool: Arc<MachinePool>,
    machine_id: i32,
    armed: bool,
}

impl ReleaseGuard {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ReleaseGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let pool = Arc::clone(&self.pool);
        let machine_id = self.machine_id;
        tokio::spawn(async move {
            if let Err(e) = pool.release(machine_id).await {
                warn!(machine_id, error = %e, "failed to release machine");
            }
        });
    }
}

fn machine_lost(what: &str, e: impl std::fmt::Display) -> JobRunError {
    JobRunError::MachineLost(format!("{what}: {e}"))
}

fn runtime_machine(machine: &DbMachine) -> RuntimeMachine {
    let platform = match machine.platform {
        MachinePlatform::Windows => Platform::Windows,
        MachinePlatform::Linux => Platform::Linux,
    };
    let provider_id = machine.provider_id.clone().unwrap_or_default();
    let mut m = RuntimeMachine::new(MachineId(provider_id.clone()));
    m.set_state(MachineState::Running);
    if let Some(addr) = machine
        .ip
        .as_deref()
        .and_then(|ip| ip.parse::<IpAddr>().ok())
    {
        m.set_endpoint(Some(MachineEndpoint {
            address: addr,
            id: provider_id,
            platform,
        }));
    }
    m
}

fn snapshot_guest_plugins(snapshot: &MachineSnapshot) -> Vec<String> {
    snapshot
        .guest_plugins
        .clone()
        .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
        .unwrap_or_default()
}

/// Guest plugins of `snapshot` to run, each with its port, plus the first port;
/// empty `requested` keeps all.
fn select_guest_plugins(
    snapshot: &MachineSnapshot,
    requested: &[String],
) -> Result<(Vec<(String, u16)>, u16), JobRunError> {
    let plugins = snapshot_guest_plugins(snapshot)
        .into_iter()
        .enumerate()
        .filter(|(_, p)| requested.is_empty() || requested.contains(p))
        .map(|(i, p)| {
            u16::try_from(i)
                .ok()
                .and_then(|i| GUEST_BASE_PORT.checked_add(i))
                .map(|port| (p, port))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            JobRunError::Nonrecoverable(format!(
                "too many guest plugins in snapshot {}",
                snapshot.name
            ))
        })?;
    let Some(&(_, first_port)) = plugins.first() else {
        return Err(JobRunError::Nonrecoverable(format!(
            "no guest plugins available in snapshot {}",
            snapshot.name
        )));
    };
    Ok((plugins, first_port))
}

/// Close a native guest session, logging (not failing) on error.
async fn close_session(session: Option<Box<dyn GuestSession>>) {
    if let Some(s) = session
        && let Err(e) = s.close().await
    {
        warn!(error = %e, "failed to close guest session");
    }
}

/// Map each guest plugin's stream result to a [`PluginOutputs`] with its retry class.
fn guest_outputs(
    handles: &[(String, PluginHandle)],
    results: Vec<Result<Vec<PluginOutput>, ManagerError>>,
    cancel: &CancellationToken,
) -> Vec<PluginOutputs> {
    let canceled = cancel.is_cancelled();
    handles
        .iter()
        .zip(results)
        .map(|((name, _), r)| PluginOutputs {
            plugin: name.clone(),
            result: r.map_err(|e| match &e {
                _ if canceled => JobRunError::Canceled,
                ManagerError::Timeout(_) => JobRunError::Timeout(e.to_string()),
                ManagerError::Reported(..) => JobRunError::Nonrecoverable(e.to_string()),
                ManagerError::Transport(_) | ManagerError::Crashed(_) => {
                    JobRunError::MachineLost(e.to_string())
                }
                _ => JobRunError::Recoverable(e.to_string()),
            }),
        })
        .collect()
}

impl MachineDetonationRunner {
    async fn mark_failed(&self, guard: &mut ReleaseGuard, reason: &str) {
        guard.disarm();
        if let Err(e) = self.pool.mark_failed(guard.machine_id, reason).await {
            warn!(machine_id = guard.machine_id, error = %e, "failed to mark machine failed");
        }
    }

    /// Load the leased machine and snapshot, restore the snapshot and boot the machine.
    async fn boot_machine(
        &self,
        lease: &Lease,
        guard: &mut ReleaseGuard,
    ) -> Result<(DbMachine, MachineSnapshot), JobRunError> {
        let machine = self
            .pool
            .get(lease.machine_id)
            .await
            .map_err(|e| machine_lost("failed to load machine", e))?
            .ok_or_else(|| JobRunError::MachineLost("machine disappeared".into()))?;
        let snapshot = snapshots::fetch_snapshot_by_id(&self.db, lease.snapshot_id)
            .await
            .map_err(|e| machine_lost("failed to load snapshot", e))?
            .ok_or_else(|| JobRunError::MachineLost("snapshot disappeared".into()))?;

        if let Err(e) = self.pool.prepare_snapshot(&machine, &snapshot).await {
            let reason = format!("snapshot preparation failed: {e}");
            self.mark_failed(guard, &reason).await;
            return Err(JobRunError::MachineLost(reason));
        }
        let _permit = self
            .boot
            .acquire()
            .await
            .map_err(|e| JobRunError::Recoverable(e.to_string()))?;
        if let Err(e) = self.pool.start_and_wait(&machine).await {
            let reason = format!("machine failed to start: {e}");
            self.mark_failed(guard, &reason).await;
            return Err(JobRunError::MachineLost(reason));
        }
        Ok((machine, snapshot))
    }

    /// Register every selected guest plugin; on failure, release the ones already registered.
    async fn register_guests(
        &self,
        plugins: &[(String, u16)],
        ip: &str,
    ) -> Result<Vec<(String, PluginHandle)>, JobRunError> {
        let mut handles: Vec<(String, PluginHandle)> = Vec::with_capacity(plugins.len());
        for (name, port) in plugins {
            match self
                .manager
                .register_guest(&PluginId::new(name), format!("http://{ip}:{port}"))
                .await
            {
                Ok(h) => handles.push((name.clone(), h)),
                Err(e) => {
                    for (_, h) in handles {
                        h.release().await;
                    }
                    return Err(machine_lost(
                        &format!("failed to register guest plugin {name}"),
                        e,
                    ));
                }
            }
        }
        Ok(handles)
    }

    /// Push the sample. Returns the native session to launch through, if any.
    async fn push_sample(
        &self,
        machine: &DbMachine,
        ip: &str,
        first_port: u16,
        sample_path: &std::path::Path,
        target: &str,
    ) -> Result<Option<Box<dyn GuestSession>>, JobRunError> {
        if let Some(ResolvedTransport::Native {
            guest_access,
            transport_name,
            config,
        }) = self.transport.as_deref()
        {
            let session = guest_access
                .open_session(transport_name, &runtime_machine(machine), config)
                .await
                .map_err(|e| machine_lost("failed to open guest session", e))?;
            if let Err(e) = session.push_file(sample_path, target).await {
                if let Err(close) = session.close().await {
                    warn!(error = %close, "failed to close guest session");
                }
                return Err(machine_lost("failed to push sample", e));
            }
            return Ok(Some(session));
        }
        let mut client = GrpcClient::connect(format!("http://{ip}:{first_port}"))
            .await
            .map_err(|e| machine_lost("failed to connect to guest", e))?;
        let data = tokio::fs::read(sample_path)
            .await
            .map_err(|e| JobRunError::Recoverable(format!("failed to read sample: {e}")))?;
        let resp = client
            .push_file(target, data)
            .await
            .map_err(|e| machine_lost("failed to push sample", e))?;
        if !resp.success {
            return Err(JobRunError::MachineLost(format!(
                "guest refused sample: {}",
                resp.error_message
            )));
        }
        Ok(None)
    }

    async fn launch(
        &self,
        session: Option<&dyn GuestSession>,
        ip: &str,
        first_port: u16,
        command: &package::LaunchCommand,
    ) -> Result<(), String> {
        if let Some(session) = session {
            session
                .execute(
                    &command.command,
                    &command.args,
                    ExecOptions {
                        cwd: self.guest_sample_dir.clone(),
                        env: vec![],
                        timeout: None,
                        background: true,
                    },
                )
                .await
                .map(|_| ())
                .map_err(|e| format!("failed to launch sample: {e}"))
        } else {
            let mut client = GrpcClient::connect(format!("http://{ip}:{first_port}"))
                .await
                .map_err(|e| format!("failed to connect to guest: {e}"))?;
            client
                .execute_command(
                    &command.command,
                    &command.args,
                    None,
                    &[],
                    None,
                    ExecMode::Background,
                )
                .await
                .map(|_| ())
                .map_err(|e| format!("failed to launch sample: {e}"))
        }
    }
}

#[async_trait]
impl DetonationRunner for MachineDetonationRunner {
    async fn acquire(&self, request: &DetonationRequest, task_id: i32) -> AcquireResult {
        let req = MachineRequest {
            platform: request.platform.clone(),
            arch: request.arch.clone(),
            machine_id: request.machine_id,
            snapshot_id: request.snapshot_id,
            snapshot_name: request.snapshot_name.clone(),
            tags: request.tags.clone(),
            required_guest_plugins: request.guest_plugins.clone(),
        };
        match self.pool.acquire_for(&req, task_id).await {
            Ok(AcquireOutcome::Acquired { machine, snapshot }) => {
                if let Some(machine_id) = machine.id {
                    AcquireResult::Acquired(Lease {
                        machine_id,
                        snapshot_id: snapshot.id,
                    })
                } else {
                    warn!(task_id, "acquired machine has no id");
                    AcquireResult::Busy
                }
            }
            Ok(AcquireOutcome::Busy) => AcquireResult::Busy,
            Ok(AcquireOutcome::Unserviceable(m)) => AcquireResult::Unserviceable(m),
            Err(e) => {
                warn!(task_id, error = %e, "machine acquisition failed, will retry");
                AcquireResult::Busy
            }
        }
    }

    async fn abandon(&self, lease: Lease) {
        if let Err(e) = self.pool.release(lease.machine_id).await {
            warn!(machine_id = lease.machine_id, error = %e, "failed to release machine");
        }
    }

    async fn run(
        &self,
        lease: Lease,
        req: DetonationJobRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<PluginOutputs>, JobRunError> {
        let mut guard = ReleaseGuard {
            pool: Arc::clone(&self.pool),
            machine_id: lease.machine_id,
            armed: true,
        };
        let task_id = req.task.id;

        let (machine, snapshot) = self.boot_machine(&lease, &mut guard).await?;
        let (plugins, first_port) = select_guest_plugins(&snapshot, &req.request.guest_plugins)?;
        let ip = machine
            .ip
            .clone()
            .ok_or_else(|| JobRunError::MachineLost("machine has no IP".into()))?;

        let target = package::guest_file_name(
            req.base_config
                .get(job_config::ARTIFACT_NAME)
                .map(String::as_str),
            &req.artifact.sha256,
            &req.artifact.artifact_type,
            &req.artifact.mime,
        );
        let session = self
            .push_sample(&machine, &ip, first_port, &req.sample_path, &target)
            .await?;
        info!(task_id, machine_id = lease.machine_id, target = %target, "sample pushed to guest");

        let handles = match self.register_guests(&plugins, &ip).await {
            Ok(h) => h,
            Err(e) => {
                close_session(session).await;
                return Err(e);
            }
        };

        let timeout_secs = req.request.timeout_secs;
        let stream_timeout =
            Duration::from_secs(timeout_secs.saturating_add(GUEST_STREAM_SLACK_SECS));
        let mut ready_rxs = Vec::with_capacity(handles.len());
        let streams = futures::future::join_all(handles.iter().map(|(_, h)| {
            let (ready_tx, ready_rx) = oneshot::channel();
            ready_rxs.push(ready_rx);
            let mut config = req.base_config.clone();
            config.insert(job_config::LAUNCH_OWNER.into(), "daemon".into());
            config.insert(
                job_config::ANALYSIS_TIMEOUT.into(),
                timeout_secs.to_string(),
            );
            h.execute_task(task_id, &target, config, stream_timeout, Some(ready_tx))
        }));

        let control = async {
            let ready_deadline = Instant::now() + GUEST_READY_TIMEOUT;
            for (rx, (name, _)) in ready_rxs.into_iter().zip(&handles) {
                if tokio::time::timeout_at(ready_deadline, rx).await.is_err() {
                    warn!(task_id, plugin = %name, "guest plugin not ready, launching without it");
                }
            }
            let launch_error = if cancel.is_cancelled() {
                None
            } else {
                match package::launch_command(
                    &req.artifact.artifact_type,
                    &machine.platform,
                    &target,
                    req.request.package.as_deref(),
                    req.request.dll_function.as_deref(),
                ) {
                    Err(e) => Some(e),
                    Ok(command) => self
                        .launch(session.as_deref(), &ip, first_port, &command)
                        .await
                        .err(),
                }
            };
            if launch_error.is_none() {
                tokio::select! {
                    () = tokio::time::sleep(Duration::from_secs(timeout_secs)) => {}
                    () = cancel.cancelled() => {}
                }
            }
            for (name, h) in &handles {
                if let Err(e) = h.stop_task(task_id).await {
                    warn!(task_id, plugin = %name, error = %e, "failed to stop guest plugin");
                }
            }
            launch_error
        };

        let (results, launch_error) = tokio::join!(streams, control);
        close_session(session).await;

        let mut out = guest_outputs(&handles, results, &cancel);
        if let Some(e) = launch_error {
            out.push(PluginOutputs {
                plugin: LAUNCH_FAILURE_PLUGIN.to_string(),
                result: Err(JobRunError::Nonrecoverable(e)),
            });
        }

        for (_, h) in handles {
            h.release().await;
        }
        drop(guard);
        Ok(out)
    }
}
