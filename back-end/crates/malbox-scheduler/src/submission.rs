//! Task creation and sample rows for artifacts.

use std::path::Path;

use malbox_database::repositories::artifacts::{self, NewArtifact, TaskArtifact};
use malbox_database::repositories::machinery::MachinePlatform;
use malbox_database::repositories::samples::{self, Sample, SampleEntity};
use malbox_database::repositories::tasks::{self, NewTask, Task};
use malbox_utils::hashing;
use malbox_utils::identify::Identification;
use serde_json::json;
use uuid::Uuid;

use crate::OrchestratorCore;
use crate::config::{EffectiveLimits, to_i32, to_i64};
use crate::error::{Result, SchedulerError};
use crate::params::{DetonateMode, SubmissionParams};
use crate::plan::{DetonationRequest, SUBMISSION_PROFILE, TaskSelection, selected_guest_plugins};
use crate::routing::RoutingTable;

/// A validated submission: the root sample plus the user's choices.
#[derive(Debug)]
pub struct NewSubmission {
    /// Stored `samples` row of the submitted file.
    pub sample: SampleEntity,
    /// Detected artifact type and MIME of the submitted file.
    pub identification: Identification,
    /// Original file name or target shown to users.
    pub target: String,
    /// Plugins the user selected; empty means every routed plugin.
    pub plugins: Vec<String>,
    /// Plugins the user excluded.
    pub excluded_plugins: Vec<String>,
    /// Detonation timeout, in seconds.
    pub timeout: i64,
    /// Scheduling priority; higher runs first.
    pub priority: i64,
    /// Requested guest platform for an explicit root detonation.
    pub platform: Option<MachinePlatform>,
    /// Requested snapshot for an explicit root detonation.
    pub snapshot_id: Option<Uuid>,
    /// Requested machine for an explicit root detonation.
    pub machine_id: Option<i32>,
    /// User that submitted the task.
    pub owner: Option<String>,
    /// Free-form labels attached to the task.
    pub tags: Option<Vec<String>>,
    /// Analysis options chosen at submission time.
    pub params: SubmissionParams,
    /// Task that produced this submission, for resubmitted children.
    pub parent_task_id: Option<i32>,
}

/// Path of a blob relative to the data directory, as stored in
/// `task_artifacts.storage_path`.
pub(crate) fn relative_storage_path(data_dir: &Path, path: &Path) -> String {
    path.strip_prefix(data_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// Insert the task and its root artifact in one transaction.
///
/// # Errors
///
/// Fails when the params cannot be encoded, the sample blob path is invalid,
/// or a database insert fails.
pub async fn create_task(
    core: &OrchestratorCore,
    sub: NewSubmission,
) -> Result<(Task, TaskArtifact)> {
    let limits = EffectiveLimits::resolve(&core.settings, &sub.params);
    let deadline = limits.deadline_from_now();
    let shared_data = if sub.params.initial_data.passwords.is_empty() {
        json!({})
    } else {
        json!({ "passwords": sub.params.initial_data.passwords })
    };
    let params = serde_json::to_value(&sub.params)
        .map_err(|e| SchedulerError::Internal(format!("failed to encode params: {e}")))?;
    let blob = core.sample_store.path(&sub.sample.sha256)?;
    let storage_path = relative_storage_path(&core.data_dir, &blob);

    let mut tx = core.db.begin().await?;
    let task = tasks::insert_task(
        &mut tx,
        &NewTask {
            target: &sub.target,
            plugins: &sub.plugins,
            excluded_plugins: &sub.excluded_plugins,
            timeout: sub.timeout,
            priority: sub.priority,
            machine_id: sub.machine_id,
            platform: sub.platform,
            snapshot_id: sub.snapshot_id,
            params: &params,
            max_depth: to_i32(limits.max_depth),
            max_artifacts: to_i32(limits.max_artifacts),
            max_detonations: to_i32(limits.max_detonations),
            max_total_bytes: to_i64(limits.max_total_bytes),
            deadline,
            sample_id: Some(sub.sample.id),
            parent_task_id: sub.parent_task_id,
            owner: sub.owner.as_deref(),
            tags: sub.tags.as_deref(),
            shared_data: &shared_data,
        },
    )
    .await?;
    let (root, _) = artifacts::insert_or_get_artifact(
        &mut tx,
        &NewArtifact {
            task_id: task.id,
            sha256: &sub.sample.sha256,
            size_bytes: sub.sample.file_size,
            storage_path: Some(&storage_path),
            analyze: true,
            sample_id: Some(sub.sample.id),
            depth: 0,
            artifact_type: &sub.identification.artifact_type,
            mime: &sub.identification.mime,
            state: "pending",
            drop_reason: None,
            priority: task.priority,
            analyze_requested: false,
            dynamic_recursion_bypass: false,
        },
    )
    .await?;
    tx.commit().await?;
    Ok((task, root))
}

/// The `samples` row for an artifact's blob, created if missing.
///
/// # Errors
///
/// Fails when the artifact's blob is missing or unreadable, hashing panics, or
/// the database insert fails.
pub async fn ensure_sample_for_artifact(
    core: &OrchestratorCore,
    artifact: &TaskArtifact,
) -> Result<SampleEntity> {
    let path = core.sample_store.path(&artifact.sha256)?;
    let data = match tokio::fs::read(&path).await {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SchedulerError::Internal("artifact blob missing".into()));
        }
        Err(e) => return Err(SchedulerError::Storage(e.into())),
    };
    let identify = core.identify;
    let sha256 = artifact.sha256.clone();
    let sample = tokio::task::spawn_blocking(move || Sample {
        file_size: i64::try_from(data.len()).unwrap_or(i64::MAX),
        file_type: identify(&data, None).description,
        md5: hashing::get_md5(&data),
        crc32: hashing::get_crc32(&data),
        sha1: hashing::get_sha1(&data),
        sha256,
        sha512: hashing::get_sha512(&data),
        ssdeep: "not-available".to_string(),
    })
    .await
    .map_err(|e| SchedulerError::Internal(format!("hashing task failed: {e}")))?;
    Ok(samples::insert_sample(&core.db, sample).await?)
}

/// The root detonation the submission asked for explicitly (platform,
/// snapshot or machine), if any.
pub(crate) fn explicit_detonation(
    task: &Task,
    params: &SubmissionParams,
    routing: &RoutingTable,
) -> Option<DetonationRequest> {
    if params.detonate == DetonateMode::Never
        || (task.platform.is_none() && task.snapshot_id.is_none() && task.machine_id.is_none())
    {
        return None;
    }
    let selection = TaskSelection {
        selected: task.plugins.clone(),
        excluded: task.excluded_plugins.clone(),
    };
    Some(DetonationRequest {
        profile: SUBMISSION_PROFILE.to_string(),
        platform: task.platform.clone(),
        arch: None,
        machine_id: task.machine_id,
        snapshot_id: task.snapshot_id,
        snapshot_name: None,
        tags: Vec::new(),
        guest_plugins: selected_guest_plugins(&selection, routing),
        timeout_secs: u64::try_from(task.timeout).unwrap_or(0),
        package: params.detonation.package.clone(),
        dll_function: params.detonation.dll_function.clone(),
    })
}

/// Parse the stored `tasks.params`.
pub(crate) fn task_params(task: &Task) -> SubmissionParams {
    SubmissionParams::from_json(&task.params)
}
