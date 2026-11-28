//! Task submission, inspection, and artifact retrieval endpoints.

use super::ApiClient;
use crate::error::{CliError, Result};
use reqwest::multipart;
use serde::Deserialize;
use std::path::Path;

/// Response of `POST /v1/tasks` identifying the newly created task.
#[derive(Deserialize, Debug)]
pub struct TaskResponse {
    /// Identifier of the created task.
    pub task_id: i32,
}

/// Summary of an analysis task as listed by the daemon.
#[derive(Deserialize, Debug)]
pub struct TaskInfo {
    /// Task identifier.
    pub id: i32,
    /// Lifecycle status of the task.
    pub status: String,
    /// Submitted sample name or target.
    pub target: String,
    /// Guest platform the task runs on.
    pub platform: String,
    /// Analysis timeout in seconds.
    pub timeout: i64,
    /// Scheduling priority; higher runs first.
    pub priority: i64,
    /// Free-form tags attached at submission.
    pub tags: Option<Vec<String>>,
    /// Submitting user, if recorded.
    pub owner: Option<String>,
    /// Machine the task ran on, once assigned.
    pub machine_id: Option<i32>,
    /// Creation timestamp.
    pub created_on: String,
    /// Completion timestamp, once finished.
    pub completed_on: Option<String>,
    /// Number of artifacts in the task tree.
    pub artifact_count: i32,
    /// Number of failed jobs.
    pub error_count: i32,
    /// Whether some jobs failed or were skipped.
    pub partial: bool,
    /// Artifact of the submitted sample.
    pub root_artifact_id: Option<i64>,
}

/// `GET /v1/tasks` page; the cursor fields are not needed by the CLI.
#[derive(Deserialize, Debug)]
struct PaginatedTasks {
    items: Vec<TaskInfo>,
}

/// Classification assigned to an artifact by analysis.
#[derive(Deserialize, Debug, Clone)]
pub struct VerdictInfo {
    /// Verdict label such as `malicious` or `benign`.
    pub classification: String,
    /// Optional numeric confidence score.
    pub score: Option<i16>,
}

/// One artifact of a task's tree (`GET /v1/tasks/{id}/tree`).
#[derive(Deserialize, Debug, Clone)]
pub struct ArtifactInfo {
    /// Artifact identifier.
    pub id: i64,
    /// SHA-256 digest of the content, hex encoded.
    pub sha256: String,
    /// Content size in bytes.
    pub size_bytes: i64,
    /// Whether the artifact is selected for analysis.
    pub analyze: bool,
    /// Deduplicated sample record, if any.
    pub sample_id: Option<i64>,
    /// Distance from the root artifact.
    pub depth: i32,
    /// Artifact kind (sample, dropped file, ...).
    pub artifact_type: String,
    /// Detected MIME type.
    pub mime: String,
    /// Processing state.
    pub state: String,
    /// Why the artifact was not analyzed, if dropped.
    pub drop_reason: Option<String>,
    /// Scheduling priority of its jobs.
    pub priority: i64,
    /// Who or what canceled processing, if canceled.
    pub canceled_by: Option<String>,
    /// Task created by promoting this artifact, if any.
    pub promoted_task_id: Option<i32>,
    /// Display name of the artifact.
    pub name: String,
    /// Relation to its parent artifact.
    pub relation: String,
    /// Analysis verdict, once available.
    pub verdict: Option<VerdictInfo>,
    /// API path to download the content, if retained.
    pub content_url: Option<String>,
}

/// Parent/child relation between two artifacts of a task tree.
#[derive(Deserialize, Debug, Clone)]
pub struct EdgeInfo {
    /// Artifact that produced the child.
    pub parent_artifact_id: i64,
    /// Artifact that was produced.
    pub child_artifact_id: i64,
    /// Kind of relation (extracted, dropped, ...).
    pub relation: String,
    /// Name of the child within the parent, if any.
    pub name: Option<String>,
    /// Job that discovered the relation.
    pub source_job_id: Option<i64>,
    /// Plugin that discovered the relation.
    pub source_plugin: Option<String>,
}

/// One analyzer job run against an artifact.
#[derive(Deserialize, Debug, Clone)]
pub struct JobInfo {
    /// Job identifier.
    pub id: i64,
    /// Artifact the job analyzes.
    pub artifact_id: i64,
    /// Job kind.
    pub kind: String,
    /// Analyzer plugin that runs the job.
    pub analyzer: String,
    /// Pipeline stage of the job.
    pub stage: String,
    /// Execution state.
    pub state: String,
    /// Attempts made so far.
    pub attempts: i16,
    /// Maximum attempts before giving up.
    pub max_attempts: i16,
    /// Times the job was manually rerun.
    pub reruns: i16,
    /// Failure category, if failed.
    pub error_class: Option<String>,
    /// Failure message, if failed.
    pub error_message: Option<String>,
    /// Earlier job whose result was reused.
    pub cached_from_job_id: Option<i64>,
    /// Machine the job ran on, if any.
    pub machine_id: Option<i32>,
    /// Start timestamp.
    pub started_on: Option<String>,
    /// Completion timestamp.
    pub completed_on: Option<String>,
}

/// Full artifact tree of a task with its edges and jobs.
#[derive(Deserialize, Debug, Clone)]
pub struct TaskTree {
    /// Task the tree belongs to.
    pub task_id: i32,
    /// Artifact of the submitted sample.
    pub root_artifact_id: Option<i64>,
    /// All artifacts of the task.
    pub artifacts: Vec<ArtifactInfo>,
    /// Relations between artifacts.
    pub edges: Vec<EdgeInfo>,
    /// Jobs run against the artifacts.
    pub jobs: Vec<JobInfo>,
}

/// Multipart fields accepted by `POST /v1/tasks`.
#[derive(Debug)]
pub struct SubmitTaskRequest {
    /// Local path of the sample file to upload.
    pub file_path: String,
    /// Analysis timeout in seconds.
    pub timeout: Option<i64>,
    /// Scheduling priority; higher runs first.
    pub priority: Option<i64>,
    /// Free-form tags to attach.
    pub tags: Option<Vec<String>>,
    /// Submitting user.
    pub owner: Option<String>,
    /// `linux` or `windows`. With `snapshot_id`, the task runs on a machine.
    pub platform: Option<String>,
    /// Machine snapshot UUID to analyse on.
    pub snapshot_id: Option<String>,
    /// Specific machine to detonate on.
    pub machine_id: Option<i32>,
    /// Plugins to run exclusively.
    pub plugins: Option<Vec<String>>,
    /// Plugins to skip.
    pub excluded_plugins: Option<Vec<String>>,
    /// Sample file name inside the guest; defaults to the uploaded name.
    pub target_filename: Option<String>,
    /// JSON-encoded submission parameters (`SubmissionParams`).
    pub params: Option<serde_json::Value>,
}

impl ApiClient {
    /// Lists analysis tasks (first page only).
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`] if the request fails or the response cannot be
    /// decoded, and [`CliError::Api`] if the daemon answers with an error status.
    pub async fn list_tasks(&self) -> Result<Vec<TaskInfo>> {
        let response = self.client.get(self.url("/v1/tasks")).send().await?;
        let response = self.check_response(response).await?;
        let page: PaginatedTasks = response.json().await?;
        Ok(page.items)
    }

    /// Fetches the summary of one task.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`] if the request fails or the response cannot be
    /// decoded, and [`CliError::Api`] if the daemon answers with an error status.
    pub async fn get_task(&self, task_id: i32) -> Result<TaskInfo> {
        let response = self
            .client
            .get(self.url(&format!("/v1/tasks/{task_id}")))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Requests cancellation of a task.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`] if the request fails or the response cannot be
    /// decoded, and [`CliError::Api`] if the daemon answers with an error status.
    pub async fn cancel_task(&self, task_id: i32) -> Result<()> {
        let response = self
            .client
            .post(self.url(&format!("/v1/tasks/{task_id}/cancel")))
            .send()
            .await?;
        self.check_response(response).await?;
        Ok(())
    }

    /// Fetches the artifact tree, edges, and jobs of a task.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`] if the request fails or the response cannot be
    /// decoded, and [`CliError::Api`] if the daemon answers with an error status.
    pub async fn get_task_tree(&self, task_id: i32) -> Result<TaskTree> {
        let response = self
            .client
            .get(self.url(&format!("/v1/tasks/{task_id}/tree")))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Downloads the raw content of an artifact of a task.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`] if the request fails or the response cannot be
    /// decoded, and [`CliError::Api`] if the daemon answers with an error status.
    pub async fn get_artifact_content(&self, task_id: i32, artifact_id: i64) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(self.url(&format!(
                "/v1/tasks/{task_id}/artifacts/{artifact_id}/content"
            )))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.bytes().await?.to_vec())
    }

    /// Uploads a sample file and submits it as a new analysis task.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Io`] if the sample file cannot be read, [`CliError::Http`]
    /// if the request fails or the response cannot be decoded, and [`CliError::Api`]
    /// if the daemon rejects the submission.
    pub async fn submit_task(&self, request: SubmitTaskRequest) -> Result<TaskResponse> {
        let file_bytes = tokio::fs::read(&request.file_path)
            .await
            .map_err(CliError::Io)?;

        let file_name = Path::new(&request.file_path).file_name().map_or_else(
            || "data.bin".to_string(),
            |n| n.to_string_lossy().to_string(),
        );

        let file_part = multipart::Part::bytes(file_bytes).file_name(file_name);

        let mut form = multipart::Form::new().part("file", file_part);

        if let Some(timeout) = request.timeout {
            form = form.text("timeout", timeout.to_string());
        }
        if let Some(priority) = request.priority {
            form = form.text("priority", priority.to_string());
        }
        if let Some(tags) = request.tags {
            form = form.text("tags", tags.join(","));
        }
        if let Some(owner) = request.owner {
            form = form.text("owner", owner);
        }
        if let Some(platform) = request.platform {
            form = form.text("platform", platform);
        }
        if let Some(snapshot_id) = request.snapshot_id {
            form = form.text("snapshot_id", snapshot_id);
        }
        if let Some(machine_id) = request.machine_id {
            form = form.text("machine_id", machine_id.to_string());
        }
        if let Some(plugins) = request.plugins {
            form = form.text("plugins", plugins.join(","));
        }
        if let Some(excluded) = request.excluded_plugins {
            form = form.text("excluded_plugins", excluded.join(","));
        }
        if let Some(target_filename) = request.target_filename {
            form = form.text("target_filename", target_filename);
        }
        if let Some(params) = request.params {
            form = form.text("params", params.to_string());
        }

        let response = self
            .client
            .post(self.url("/v1/tasks"))
            .multipart(form)
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }
}
