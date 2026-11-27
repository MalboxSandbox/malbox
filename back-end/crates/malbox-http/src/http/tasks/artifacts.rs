//! Artifact tree, job listing and per-artifact control endpoints.

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use malbox_database::repositories::{
    artifacts::{
        ArtifactEdge, TaskArtifact, fetch_artifact, fetch_task_artifacts, fetch_task_edges,
        set_promoted_task,
    },
    jobs::{Job, fetch_task_jobs},
    plugin_reports::{DbClassification, fetch_plugin_reports_for_task},
    tasks::{Task, fetch_task},
};
use malbox_scheduler::{DescendantPolicy, ensure_sample_for_artifact, package::sanitize_file_name};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio_util::io::ReaderStream;

use super::super::AppState;
use super::control_error;
use super::create::{SubmitOptionsBody, created_response, identify_blocking, submit};
use crate::http::{Result, error::Error};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks/{id}/tree", get(get_tree))
        .route("/v1/tasks/{id}/jobs", get(get_jobs))
        .route(
            "/v1/tasks/{id}/artifacts/{artifact_id}",
            axum::routing::patch(patch_artifact),
        )
        .route(
            "/v1/tasks/{id}/artifacts/{artifact_id}/content",
            get(get_artifact_content),
        )
        .route(
            "/v1/tasks/{id}/artifacts/{artifact_id}/cancel",
            post(cancel_artifact),
        )
        .route(
            "/v1/tasks/{id}/artifacts/{artifact_id}/rescan",
            post(rescan_artifact),
        )
        .route(
            "/v1/tasks/{id}/artifacts/{artifact_id}/promote",
            post(promote_artifact),
        )
        .route("/v1/tasks/{id}/jobs/{job_id}/cancel", post(cancel_job))
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ArtifactVerdictDto {
    classification: String,
    score: Option<i16>,
}

#[derive(Serialize)]
struct ArtifactDto {
    id: i64,
    sha256: String,
    size_bytes: i64,
    analyze: bool,
    sample_id: Option<i64>,
    depth: i32,
    artifact_type: String,
    mime: String,
    state: String,
    drop_reason: Option<String>,
    priority: i64,
    canceled_by: Option<String>,
    promoted_task_id: Option<i32>,
    name: String,
    relation: String,
    verdict: Option<ArtifactVerdictDto>,
    content_url: Option<String>,
}

#[derive(Serialize)]
struct EdgeDto {
    parent_artifact_id: i64,
    child_artifact_id: i64,
    relation: String,
    name: Option<String>,
    source_job_id: Option<i64>,
    source_plugin: Option<String>,
}

impl From<ArtifactEdge> for EdgeDto {
    fn from(e: ArtifactEdge) -> Self {
        EdgeDto {
            parent_artifact_id: e.parent_artifact_id,
            child_artifact_id: e.child_artifact_id,
            relation: e.relation,
            name: e.name,
            source_job_id: e.source_job_id,
            source_plugin: e.source_plugin,
        }
    }
}

#[derive(Serialize)]
struct JobDto {
    id: i64,
    artifact_id: i64,
    kind: String,
    analyzer: String,
    stage: String,
    state: String,
    attempts: i16,
    max_attempts: i16,
    reruns: i16,
    error_class: Option<String>,
    error_message: Option<String>,
    cached_from_job_id: Option<i64>,
    machine_id: Option<i32>,
    started_on: Option<String>,
    completed_on: Option<String>,
}

fn rfc3339(t: Option<OffsetDateTime>) -> Option<String> {
    t.and_then(|t| t.format(&Rfc3339).ok())
}

impl From<Job> for JobDto {
    fn from(j: Job) -> Self {
        JobDto {
            id: j.id,
            artifact_id: j.artifact_id,
            kind: j.kind,
            analyzer: j.analyzer,
            stage: j.stage,
            state: j.state,
            attempts: j.attempts,
            max_attempts: j.max_attempts,
            reruns: j.reruns,
            error_class: j.error_class,
            error_message: j.error_message,
            cached_from_job_id: j.cached_from_job_id,
            machine_id: j.machine_id,
            started_on: rfc3339(j.started_on),
            completed_on: rfc3339(j.completed_on),
        }
    }
}

#[derive(Serialize)]
struct TreeResponse {
    task_id: i32,
    root_artifact_id: Option<i64>,
    artifacts: Vec<ArtifactDto>,
    edges: Vec<EdgeDto>,
    jobs: Vec<JobDto>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn load_task(state: &AppState, task_id: i32) -> Result<Task> {
    fetch_task(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)
}

async fn load_artifact(state: &AppState, task_id: i32, artifact_id: i64) -> Result<TaskArtifact> {
    match fetch_artifact(&state.pool, artifact_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
    {
        Some(a) if a.task_id == task_id => Ok(a),
        _ => Err(Error::NotFound),
    }
}

/// `(name, relation)` of an artifact: its earliest incoming edge, or the
/// task target and `root` for the root.
fn artifact_label(
    task: &Task,
    artifact: &TaskArtifact,
    edges: &[ArtifactEdge],
) -> (String, String) {
    if artifact.depth == 0 {
        return (task.target.clone(), "root".to_string());
    }
    match edges
        .iter()
        .filter(|e| e.child_artifact_id == artifact.id)
        .min_by_key(|e| e.created_on)
    {
        Some(e) => (
            e.name.clone().unwrap_or_else(|| artifact.sha256.clone()),
            e.relation.clone(),
        ),
        None => (artifact.sha256.clone(), "output".to_string()),
    }
}

fn content_url(task_id: i32, a: &TaskArtifact) -> Option<String> {
    a.storage_path
        .as_ref()
        .map(|_| format!("/v1/tasks/{}/artifacts/{}/content", task_id, a.id))
}

// ---------------------------------------------------------------------------
// Read endpoints
// ---------------------------------------------------------------------------

async fn get_tree(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> Result<Json<TreeResponse>> {
    let task = load_task(&state, task_id).await?;
    let db = |e: malbox_database::error::DatabaseError| Error::Internal(e.to_string());
    let artifacts = fetch_task_artifacts(&state.pool, task_id)
        .await
        .map_err(db)?;
    let edges = fetch_task_edges(&state.pool, task_id).await.map_err(db)?;
    let jobs = fetch_task_jobs(&state.pool, task_id).await.map_err(db)?;
    let reports = fetch_plugin_reports_for_task(&state.pool, task_id)
        .await
        .map_err(db)?;

    let mut verdicts: HashMap<i64, (Option<DbClassification>, Option<i16>)> = HashMap::new();
    for r in &reports {
        let entry = verdicts.entry(r.artifact_id).or_insert((None, None));
        if let Some(c) = r.classification
            && entry.0.is_none_or(|w| c.severity() > w.severity())
        {
            entry.0 = Some(c);
        }
        if let Some(s) = r.score {
            entry.1 = Some(entry.1.map_or(s, |m| m.max(s)));
        }
    }

    let root_artifact_id = artifacts.iter().find(|a| a.depth == 0).map(|a| a.id);
    let artifact_dtos = artifacts
        .iter()
        .map(|a| {
            let (name, relation) = artifact_label(&task, a, &edges);
            let verdict = verdicts.get(&a.id).and_then(|(c, s)| {
                c.map(|c| ArtifactVerdictDto {
                    classification: c.label().to_string(),
                    score: *s,
                })
            });
            ArtifactDto {
                id: a.id,
                sha256: a.sha256.clone(),
                size_bytes: a.size_bytes,
                analyze: a.analyze,
                sample_id: a.sample_id,
                depth: a.depth,
                artifact_type: a.artifact_type.clone(),
                mime: a.mime.clone(),
                state: a.state.clone(),
                drop_reason: a.drop_reason.clone(),
                priority: a.priority,
                canceled_by: a.canceled_by.clone(),
                promoted_task_id: a.promoted_task_id,
                name,
                relation,
                verdict,
                content_url: content_url(task_id, a),
            }
        })
        .collect();

    Ok(Json(TreeResponse {
        task_id,
        root_artifact_id,
        artifacts: artifact_dtos,
        edges: edges.into_iter().map(EdgeDto::from).collect(),
        jobs: jobs.into_iter().map(JobDto::from).collect(),
    }))
}

async fn get_jobs(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> Result<Json<Vec<JobDto>>> {
    load_task(&state, task_id).await?;
    let jobs = fetch_task_jobs(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    Ok(Json(jobs.into_iter().map(JobDto::from).collect()))
}

async fn get_artifact_content(
    State(state): State<AppState>,
    Path((task_id, artifact_id)): Path<(i32, i64)>,
) -> Result<Response> {
    let task = load_task(&state, task_id).await?;
    let artifact = load_artifact(&state, task_id, artifact_id).await?;
    if artifact.storage_path.is_none() {
        return Err(Error::NotFound);
    }
    let edges = fetch_task_edges(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let (name, _) = artifact_label(&task, &artifact, &edges);

    let unavailable = || Error::Internal("artifact file unavailable".to_string());
    let abs_path = state
        .sample_store
        .path(&artifact.sha256)
        .map_err(|_| unavailable())?;
    let canonical = tokio::fs::canonicalize(&abs_path)
        .await
        .map_err(|_| unavailable())?;
    let data_dir = tokio::fs::canonicalize(&state.config.paths.data_dir)
        .await
        .map_err(|_| unavailable())?;
    if !canonical.starts_with(&data_dir) {
        tracing::warn!(
            artifact_id,
            path = %canonical.display(),
            "path traversal blocked: artifact path escapes data directory"
        );
        return Err(Error::Forbidden);
    }
    let file = tokio::fs::File::open(&canonical)
        .await
        .map_err(|_| unavailable())?;
    let body = Body::from_stream(ReaderStream::new(file));

    let mut file_name = sanitize_file_name(&name);
    if file_name.is_empty() {
        file_name = artifact.sha256.clone();
    }
    let disposition = format!("attachment; filename=\"{}\"", file_name);

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, artifact.mime.clone()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CONTENT_LENGTH, artifact.size_bytes.to_string()),
        ],
        body,
    )
        .into_response())
}

// ---------------------------------------------------------------------------
// Control endpoints
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct CancelArtifactBody {
    #[serde(default)]
    descendants: DescendantPolicy,
}

async fn cancel_artifact(
    State(state): State<AppState>,
    Path((task_id, artifact_id)): Path<(i32, i64)>,
    body: Option<Json<CancelArtifactBody>>,
) -> Result<Json<serde_json::Value>> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    state
        .orchestrator
        .cancel_artifact(task_id, artifact_id, body.descendants)
        .await
        .map_err(control_error)?;
    Ok(Json(serde_json::json!({ "status": "canceled" })))
}

async fn cancel_job(
    State(state): State<AppState>,
    Path((task_id, job_id)): Path<(i32, i64)>,
) -> Result<Json<serde_json::Value>> {
    state
        .orchestrator
        .cancel_job(task_id, job_id)
        .await
        .map_err(control_error)?;
    Ok(Json(serde_json::json!({ "status": "canceled" })))
}

#[derive(Deserialize)]
struct PatchArtifactBody {
    priority: i64,
    #[serde(default)]
    subtree: bool,
}

async fn patch_artifact(
    State(state): State<AppState>,
    Path((task_id, artifact_id)): Path<(i32, i64)>,
    Json(body): Json<PatchArtifactBody>,
) -> Result<Json<serde_json::Value>> {
    state
        .orchestrator
        .set_artifact_priority(task_id, artifact_id, body.priority, body.subtree)
        .await
        .map_err(control_error)?;
    Ok(Json(
        serde_json::json!({ "status": "updated", "priority": body.priority }),
    ))
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct RescanBody {
    #[serde(default)]
    plugins: Vec<String>,
    #[serde(default = "default_true")]
    ignore_cache: bool,
}

impl Default for RescanBody {
    fn default() -> Self {
        RescanBody {
            plugins: Vec::new(),
            ignore_cache: true,
        }
    }
}

async fn rescan_artifact(
    State(state): State<AppState>,
    Path((task_id, artifact_id)): Path<(i32, i64)>,
    body: Option<Json<RescanBody>>,
) -> Result<Response> {
    let body = body.map(|Json(b)| b).unwrap_or_default();
    state
        .orchestrator
        .rescan_artifact(task_id, artifact_id, body.plugins, body.ignore_cache)
        .await
        .map_err(control_error)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "status": "queued" })),
    )
        .into_response())
}

#[tracing::instrument(skip_all, fields(task_id = tracing::field::Empty))]
async fn promote_artifact(
    State(state): State<AppState>,
    Path((task_id, artifact_id)): Path<(i32, i64)>,
    body: Option<Json<SubmitOptionsBody>>,
) -> Result<Response> {
    let options = body.map(|Json(b)| b).unwrap_or_default();
    let task = load_task(&state, task_id).await?;
    let artifact = load_artifact(&state, task_id, artifact_id).await?;
    if artifact.storage_path.is_none() {
        return Err(Error::Conflict(
            "artifact has no stored content".to_string(),
        ));
    }

    let sample = ensure_sample_for_artifact(state.orchestrator.core(), &artifact)
        .await
        .map_err(|e| Error::Internal(format!("Failed to prepare sample: {}", e)))?;

    let edges = fetch_task_edges(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let (name, _) = artifact_label(&task, &artifact, &edges);
    let target = options.target_filename.clone().unwrap_or(name);

    let path = state
        .sample_store
        .path(&artifact.sha256)
        .map_err(|e| Error::Internal(format!("Failed to resolve artifact path: {}", e)))?;
    let data = tokio::fs::read(&path)
        .await
        .map_err(|_| Error::Conflict("artifact has no stored content".to_string()))?;
    let identification = identify_blocking(Bytes::from(data), Some(target.clone())).await?;

    let (new_task, root) = submit(
        &state,
        sample.clone(),
        identification,
        target,
        options.into(),
        Some(task_id),
    )
    .await?;

    set_promoted_task(&state.pool, artifact.id, new_task.id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;

    Ok(created_response(&new_task, &root, sample))
}
