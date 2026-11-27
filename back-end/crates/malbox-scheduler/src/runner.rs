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
use malbox_plugin_internal::transport::daemon::GrpcClient;
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
const GUEST_READY_TIMEOUT: Duration = Duration::from_secs(60);
/// Extra stream time beyond the observation window: READY wait, launch,
/// and the 120 s collection grace after the stop request.
const GUEST_STREAM_SLACK_SECS: u64 = 180;

/// Pseudo plugin name of the entry a detonation runner appends when the
/// sample could not be launched. Its error fails the job as
/// non-recoverable while the real plugins' outputs are still ingested.
pub const LAUNCH_FAILURE_PLUGIN: &str = "malbox.launch";

pub struct HostJobRequest {
    pub task_id: i32,
    pub job: Job,
    pub spec: AnalyzerSpec,
    pub sample_path: PathBuf,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Error)]
pub enum JobRunError {
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Recoverable(String),
    #[error("{0}")]
    Nonrecoverable(String),
    #[error("{0}")]
    MachineLost(String),
    #[error("canceled")]
    Canceled,
}

#[async_trait]
pub trait HostJobRunner: Send + Sync {
    async fn run(
        &self,
        req: HostJobRequest,
        cancel: CancellationToken,
    ) -> Result<Vec<PluginOutput>, JobRunError>;
}

pub struct DetonationJobRequest {
    pub task: Task,
    pub job: Job,
    pub artifact: TaskArtifact,
    pub request: DetonationRequest,
    pub sample_path: PathBuf,
    pub base_config: HashMap<String, String>,
}

/// One guest plugin's result from a detonation.
pub struct PluginOutputs {
    pub plugin: String,
    pub result: Result<Vec<PluginOutput>, JobRunError>,
}

#[derive(Debug, Clone)]
pub struct Lease {
    pub machine_id: i32,
    pub snapshot_id: Uuid,
}

pub enum AcquireResult {
    Acquired(Lease),
    Busy,
    Unserviceable(String),
}

#[async_trait]
pub trait DetonationRunner: Send + Sync {
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
pub struct PluginManagerHostRunner {
    pub manager: Arc<PluginManager>,
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
                        _ = tokio::time::sleep(PLUGIN_ACQUIRE_RETRY) => {}
                        _ = cancel.cancelled() => return Err(JobRunError::Canceled),
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
            _ = cancel.cancelled() => None,
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
pub struct MachineDetonationRunner {
    pub db: PgPool,
    pub pool: Arc<MachinePool>,
    pub manager: Arc<PluginManager>,
    pub transport: Option<Arc<ResolvedTransport>>,
    pub boot: Arc<Semaphore>,
    pub guest_sample_dir: Option<String>,
}

impl MachineDetonationRunner {
    pub fn new(
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
            boot: Arc::new(Semaphore::new(max_concurrent_vm_boots.max(1) as usize)),
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
        let pool = self.pool.clone();
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

impl MachineDetonationRunner {
    async fn mark_failed(&self, guard: &mut ReleaseGuard, reason: &str) {
        guard.disarm();
        if let Err(e) = self.pool.mark_failed(guard.machine_id, reason).await {
            warn!(machine_id = guard.machine_id, error = %e, "failed to mark machine failed");
        }
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
        match session {
            Some(session) => session
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
                .map_err(|e| format!("failed to launch sample: {e}")),
            None => {
                let mut client = GrpcClient::connect(format!("http://{ip}:{first_port}"))
                    .await
                    .map_err(|e| format!("failed to connect to guest: {e}"))?;
                client
                    .execute_command(&command.command, &command.args, None, &[], None, true)
                    .await
                    .map(|_| ())
                    .map_err(|e| format!("failed to launch sample: {e}"))
            }
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
            Ok(AcquireOutcome::Acquired { machine, snapshot }) => match machine.id {
                Some(machine_id) => AcquireResult::Acquired(Lease {
                    machine_id,
                    snapshot_id: snapshot.id,
                }),
                None => {
                    warn!(task_id, "acquired machine has no id");
                    AcquireResult::Busy
                }
            },
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
            pool: self.pool.clone(),
            machine_id: lease.machine_id,
            armed: true,
        };
        let task_id = req.task.id;

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
            self.mark_failed(&mut guard, &reason).await;
            return Err(JobRunError::MachineLost(reason));
        }
        {
            let _permit = self
                .boot
                .acquire()
                .await
                .map_err(|e| JobRunError::Recoverable(e.to_string()))?;
            if let Err(e) = self.pool.start_and_wait(&machine).await {
                let reason = format!("machine failed to start: {e}");
                self.mark_failed(&mut guard, &reason).await;
                return Err(JobRunError::MachineLost(reason));
            }
        }

        let snapshot_plugins = snapshot_guest_plugins(&snapshot);
        let plugins: Vec<(String, u16)> = snapshot_plugins
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                req.request.guest_plugins.is_empty() || req.request.guest_plugins.contains(p)
            })
            .map(|(i, p)| (p.clone(), GUEST_BASE_PORT + i as u16))
            .collect();
        let Some(first_port) = plugins.first().map(|(_, port)| *port) else {
            return Err(JobRunError::Nonrecoverable(format!(
                "no guest plugins available in snapshot {}",
                snapshot.name
            )));
        };
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

        let mut handles: Vec<(String, PluginHandle)> = Vec::with_capacity(plugins.len());
        for (name, port) in &plugins {
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
                    if let Some(s) = session
                        && let Err(close) = s.close().await
                    {
                        warn!(error = %close, "failed to close guest session");
                    }
                    return Err(machine_lost(
                        &format!("failed to register guest plugin {name}"),
                        e,
                    ));
                }
            }
        }

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
                    machine.platform.clone(),
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
                    _ = tokio::time::sleep(Duration::from_secs(timeout_secs)) => {}
                    _ = cancel.cancelled() => {}
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
        if let Some(s) = session
            && let Err(e) = s.close().await
        {
            warn!(error = %e, "failed to close guest session");
        }

        let canceled = cancel.is_cancelled();
        let mut out: Vec<PluginOutputs> = handles
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
            .collect();
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
