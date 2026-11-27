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
pub mod admission;
pub mod cache;
pub mod config;
mod coordinator;
pub mod error;
mod executor;
pub mod ingest;
pub mod interest;
pub mod jobctx;
pub mod package;
pub mod params;
pub mod plan;
pub mod recursion;
pub mod routing;
pub mod runner;
mod running;
pub mod submission;
pub mod verdict;

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
    pub db: PgPool,
    pub sample_store: Arc<SampleStore>,
    pub result_store: Arc<ResultStore>,
    pub settings: Arc<OrchestrationConfig>,
    pub data_dir: PathBuf,
    /// `malbox_utils::identify::identify` in production.
    pub identify: fn(&[u8], Option<&str>) -> Identification,
    /// `PluginManager::emitter()` in production, `()` in tests.
    pub events: Arc<dyn TransportEmitter + Send + Sync>,
}

/// What happens to an artifact's descendants when it is cancelled.
#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DescendantPolicy {
    /// Cancel children whose every parent is cancelled.
    #[default]
    Cancel,
    /// Let children continue.
    Keep,
}

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("task not found")]
    TaskNotFound,
    #[error("artifact not found")]
    ArtifactNotFound,
    #[error("job not found")]
    JobNotFound,
    #[error("{0}")]
    Conflict(String),
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

impl OrchestratorHandle {
    pub fn routing(&self) -> &Arc<RoutingTable> {
        &self.routing
    }

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

    pub async fn cancel_task(&self, task_id: i32) -> Result<(), ControlError> {
        self.control(task_id, ControlCmd::CancelTask).await
    }

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

    pub async fn cancel_job(&self, task_id: i32, job_id: i64) -> Result<(), ControlError> {
        self.control(task_id, ControlCmd::CancelJob { job_id })
            .await
    }

    pub async fn rescan_artifact(
        &self,
        task_id: i32,
        artifact_id: i64,
        plugins: Vec<String>,
        ignore_cache: bool,
    ) -> Result<(), ControlError> {
        self.control(
            task_id,
            ControlCmd::Rescan {
                artifact_id,
                plugins,
                ignore_cache,
            },
        )
        .await
    }

    pub async fn set_artifact_priority(
        &self,
        task_id: i32,
        artifact_id: i64,
        priority: i64,
        subtree: bool,
    ) -> Result<(), ControlError> {
        self.control(
            task_id,
            ControlCmd::SetPriority {
                artifact_id,
                priority,
                subtree,
            },
        )
        .await
    }
}

/// Start the orchestrator with the production runners.
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
        manager: plugin_manager.clone(),
    });
    let detonation = Arc::new(MachineDetonationRunner::new(
        core.db.clone(),
        machine_pool.clone(),
        plugin_manager,
        transport,
        core.settings.max_concurrent_vm_boots,
    ));
    coordinator::launch(
        core,
        routing,
        host,
        detonation,
        Some(machine_pool.machine_available.clone()),
        token,
    )
    .await
}

/// Start the orchestrator with caller-supplied runners (tests).
pub async fn start_with_runners(
    core: OrchestratorCore,
    routing: Arc<RoutingTable>,
    host: Arc<dyn HostJobRunner>,
    detonation: Arc<dyn DetonationRunner>,
    token: CancellationToken,
) -> Result<(OrchestratorHandle, JoinHandle<()>)> {
    coordinator::launch(core, routing, host, detonation, None, token).await
}
