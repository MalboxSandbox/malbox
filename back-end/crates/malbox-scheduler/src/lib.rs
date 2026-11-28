//! Malbox analysis orchestrator.
//!
//! A task owns a DAG of artifacts: the submitted sample plus every file a
//! plugin extracts, drops or outputs. Each analysed artifact gets one host
//! job per routed host plugin and at most one detonation job. Jobs live in
//! Postgres and are released stage by stage; one actor per active task owns
//! every artifact and task state change, while executors claim and run jobs.

use std::path::PathBuf;
use std::sync::Arc;

use malbox_config::OrchestrationConfig;
use malbox_database::PgPool;
use malbox_plugin_internal::manager::PluginManager;
use malbox_plugin_transport::TransportEmitter;
use malbox_resources::{MachinePool, ResolvedTransport};
use malbox_utils::identify::Identification;
use malbox_utils::{ResultStore, SampleStore};
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

mod actor;
mod admission;
mod cache;
mod config;
mod coordinator;
mod error;
mod executor;
mod ingest;
mod interest;
mod jobctx;
pub mod package;
mod params;
mod plan;
mod recursion;
pub mod routing;
mod runner;
mod running;
mod submission;
mod verdict;

pub use error::{Result, SchedulerError};
pub use params::*;
pub use plan::DetonationRequest;
pub use routing::{AnalyzerSpec, DetonationProfile, RoutingTable};
pub use runner::{
    AcquireResult, DetonationJobRequest, DetonationRunner, HostJobRequest, HostJobRunner,
    JobRunError, Lease, PluginOutputs,
};
pub use submission::{NewSubmission, create_task, ensure_sample_for_artifact};

use actor::ControlCmd;
use coordinator::Command;
use executor::Executors;
use runner::{MachineDetonationRunner, PluginManagerHostRunner};
use running::RunningJobs;

/// Everything the orchestrator needs besides the plugin manager and the
/// machine pool.
#[derive(Clone)]
pub struct OrchestratorCore {
    /// Postgres pool holding tasks, artifacts and jobs.
    pub db: PgPool,
    /// Content-addressed store for samples and extracted artifacts.
    pub sample_store: Arc<SampleStore>,
    /// Store for plugin reports and other job outputs.
    pub result_store: Arc<ResultStore>,
    /// Orchestration limits, timeouts and detonation profiles.
    pub settings: Arc<OrchestrationConfig>,
    /// Root data directory; stored report paths are relative to it.
    pub data_dir: PathBuf,
    /// `malbox_utils::identify::identify` in production.
    pub identify: fn(&[u8], Option<&str>) -> Identification,
    /// `PluginManager::emitter()` in production, `()` in tests.
    pub events: Arc<dyn TransportEmitter + Send + Sync>,
}

impl std::fmt::Debug for OrchestratorCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrchestratorCore")
            .field("data_dir", &self.data_dir)
            .finish_non_exhaustive()
    }
}

/// What happens to an artifact's descendants when it is cancelled.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum DescendantPolicy {
    /// Cancel children whose every parent is cancelled.
    #[default]
    Cancel,
    /// Let children continue.
    Keep,
}

/// How a rescan treats results already in the result cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheUse {
    /// Replay cached results when a usable entry exists.
    Reuse,
    /// Run the jobs again even when a cached result exists.
    Bypass,
}

/// Which artifacts a priority change applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PriorityScope {
    /// Only the named artifact.
    Artifact,
    /// The named artifact and every descendant.
    Subtree,
}

/// Why a task control request (cancel, rescan, reprioritise) was refused.
#[derive(Debug, thiserror::Error)]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox-http to pick HTTP status codes"
)]
pub enum ControlError {
    /// No active task has the given id.
    #[error("task not found")]
    TaskNotFound,
    /// The task has no artifact with the given id.
    #[error("artifact not found")]
    ArtifactNotFound,
    /// The task has no job with the given id.
    #[error("job not found")]
    JobNotFound,
    /// The request is incompatible with the current task or artifact state.
    #[error("{0}")]
    Conflict(String),
    /// A database or bookkeeping failure while applying the request.
    #[error("internal error: {0}")]
    Internal(#[source] SchedulerError),
    /// The orchestrator stopped, or dropped the request while shutting down.
    #[error("orchestrator unavailable: {0}")]
    Unavailable(&'static str),
}

/// State shared by the coordinator, the task actors and the executors.
pub(crate) struct Shared {
    pub core: Arc<OrchestratorCore>,
    pub routing: Arc<RoutingTable>,
    pub running: RunningJobs,
    pub executors: Executors,
    pub commands: mpsc::UnboundedSender<Command>,
    pub token: CancellationToken,
}

/// Cloneable handle to a running orchestrator.
#[derive(Clone)]
pub struct OrchestratorHandle {
    commands: mpsc::UnboundedSender<Command>,
    routing: Arc<RoutingTable>,
    core: Arc<OrchestratorCore>,
}

impl std::fmt::Debug for OrchestratorHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OrchestratorHandle")
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}

impl OrchestratorHandle {
    /// Plugin routing table the orchestrator was started with.
    pub fn routing(&self) -> &Arc<RoutingTable> {
        &self.routing
    }

    /// Shared stores, database and settings used by the orchestrator.
    pub fn core(&self) -> &OrchestratorCore {
        &self.core
    }

    /// Start driving a task created with [`create_task`].
    pub fn submit(&self, task_id: i32) {
        // A closed channel means the orchestrator is shutting down; the task
        // stays pending and is picked up on the next start.
        let _ = self.commands.send(Command::Submit(task_id));
    }

    async fn control(&self, task_id: i32, cmd: ControlCmd) -> Result<(), ControlError> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::Control(task_id, cmd, tx))
            .map_err(|_| ControlError::Unavailable("orchestrator stopped"))?;
        rx.await
            .map_err(|_| ControlError::Unavailable("orchestrator dropped the request"))?
    }

    /// Cancel every open artifact and job of a task.
    ///
    /// # Errors
    ///
    /// Fails when the task is unknown, the cancel conflicts with its state, a
    /// database update fails, or the orchestrator is shutting down.
    pub async fn cancel_task(&self, task_id: i32) -> Result<(), ControlError> {
        self.control(task_id, ControlCmd::CancelTask).await
    }

    /// Cancel one artifact's jobs, handling its descendants per `descendants`.
    ///
    /// # Errors
    ///
    /// Fails when the task or artifact is unknown, a database update fails, or
    /// the orchestrator is shutting down.
    pub async fn cancel_artifact(
        &self,
        task_id: i32,
        artifact_id: i64,
        descendants: DescendantPolicy,
    ) -> Result<(), ControlError> {
        self.control(
            task_id,
            ControlCmd::CancelArtifact {
                artifact_id,
                descendants,
            },
        )
        .await
    }

    /// Cancel a single job of a task.
    ///
    /// # Errors
    ///
    /// Fails when the task or job is unknown, the job is already finished, a
    /// database update fails, or the orchestrator is shutting down.
    pub async fn cancel_job(&self, task_id: i32, job_id: i64) -> Result<(), ControlError> {
        self.control(task_id, ControlCmd::CancelJob { job_id })
            .await
    }

    /// Re-run the given plugins (all routed ones when empty) on an artifact.
    ///
    /// # Errors
    ///
    /// Fails when the task or artifact is unknown, the rescan conflicts with the
    /// artifact's state, a database update fails, or the orchestrator is shutting down.
    pub async fn rescan_artifact(
        &self,
        task_id: i32,
        artifact_id: i64,
        plugins: Vec<String>,
        cache: CacheUse,
    ) -> Result<(), ControlError> {
        self.control(
            task_id,
            ControlCmd::Rescan {
                artifact_id,
                plugins,
                cache,
            },
        )
        .await
    }

    /// Change the scheduling priority of an artifact, optionally its whole subtree.
    ///
    /// # Errors
    ///
    /// Fails when the task or artifact is unknown, a database update fails, or
    /// the orchestrator is shutting down.
    pub async fn set_artifact_priority(
        &self,
        task_id: i32,
        artifact_id: i64,
        priority: i64,
        scope: PriorityScope,
    ) -> Result<(), ControlError> {
        self.control(
            task_id,
            ControlCmd::SetPriority {
                artifact_id,
                priority,
                scope,
            },
        )
        .await
    }
}

/// Start the orchestrator with the production runners.
///
/// # Errors
///
/// Fails when the routing table cannot be built from the plugin registry and
/// detonation profiles, or when recovering persisted state from the database fails.
pub async fn start(
    core: OrchestratorCore,
    plugin_manager: Arc<PluginManager>,
    machine_pool: Arc<MachinePool>,
    transport: Option<Arc<ResolvedTransport>>,
    token: CancellationToken,
) -> Result<(OrchestratorHandle, JoinHandle<()>)> {
    let routing = Arc::new(RoutingTable::from_registry(
        &plugin_manager.registry().snapshot(),
        &core.settings.detonation_profiles,
    )?);
    let host = Arc::new(PluginManagerHostRunner {
        manager: Arc::clone(&plugin_manager),
    });
    let detonation = Arc::new(MachineDetonationRunner::new(
        core.db.clone(),
        Arc::clone(&machine_pool),
        plugin_manager,
        transport,
        core.settings.max_concurrent_vm_boots,
    ));
    coordinator::launch(
        core,
        routing,
        host,
        detonation,
        Some(Arc::clone(&machine_pool.machine_available)),
        token,
    )
    .await
}

/// Start the orchestrator with caller-supplied runners (tests).
///
/// # Errors
///
/// Fails when recovering persisted state from the database fails.
pub async fn start_with_runners(
    core: OrchestratorCore,
    routing: Arc<RoutingTable>,
    host: Arc<dyn HostJobRunner>,
    detonation: Arc<dyn DetonationRunner>,
    token: CancellationToken,
) -> Result<(OrchestratorHandle, JoinHandle<()>)> {
    coordinator::launch(core, routing, host, detonation, None, token).await
}
