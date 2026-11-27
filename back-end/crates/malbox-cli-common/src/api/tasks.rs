use super::ApiClient;
use crate::error::Result;
use reqwest::multipart;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct TaskResponse {
    pub task_id: i32,
}

#[derive(Deserialize, Debug)]
pub struct TaskInfo {
    pub id: i32,
    pub status: String,
    pub target: String,
    pub platform: String,
    pub timeout: i64,
    pub priority: i64,
    pub tags: Option<Vec<String>>,
    pub owner: Option<String>,
    pub machine_id: Option<i32>,
    pub created_on: String,
    pub completed_on: Option<String>,
    pub artifact_count: i32,
    pub error_count: i32,
    pub partial: bool,
    pub root_artifact_id: Option<i64>,
}

/// `GET /v1/tasks` page; the cursor fields are not needed by the CLI.
#[derive(Deserialize, Debug)]
struct PaginatedTasks {
    items: Vec<TaskInfo>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct VerdictInfo {
    pub classification: String,
    pub score: Option<i16>,
}

/// One artifact of a task's tree (`GET /v1/tasks/{id}/tree`).
#[derive(Deserialize, Debug, Clone)]
pub struct ArtifactInfo {
    pub id: i64,
    pub sha256: String,
    pub size_bytes: i64,
    pub analyze: bool,
    pub sample_id: Option<i64>,
    pub depth: i32,
    pub artifact_type: String,
    pub mime: String,
    pub state: String,
    pub drop_reason: Option<String>,
    pub priority: i64,
    pub canceled_by: Option<String>,
    pub promoted_task_id: Option<i32>,
    pub name: String,
    pub relation: String,
    pub verdict: Option<VerdictInfo>,
    pub content_url: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EdgeInfo {
    pub parent_artifact_id: i64,
    pub child_artifact_id: i64,
    pub relation: String,
    pub name: Option<String>,
    pub source_job_id: Option<i64>,
    pub source_plugin: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct JobInfo {
    pub id: i64,
    pub artifact_id: i64,
    pub kind: String,
    pub analyzer: String,
    pub stage: String,
    pub state: String,
    pub attempts: i16,
    pub max_attempts: i16,
    pub reruns: i16,
    pub error_class: Option<String>,
    pub error_message: Option<String>,
    pub cached_from_job_id: Option<i64>,
    pub machine_id: Option<i32>,
    pub started_on: Option<String>,
    pub completed_on: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TaskTree {
    pub task_id: i32,
    pub root_artifact_id: Option<i64>,
    pub artifacts: Vec<ArtifactInfo>,
    pub edges: Vec<EdgeInfo>,
    pub jobs: Vec<JobInfo>,
}

/// Multipart fields accepted by `POST /v1/tasks`.
pub struct SubmitTaskRequest {
    pub file_path: String,
    pub timeout: Option<i64>,
    pub priority: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub owner: Option<String>,
    /// `linux` or `windows`. With `snapshot_id`, the task runs on a machine.
    pub platform: Option<String>,
    /// Machine snapshot UUID to analyse on.
    pub snapshot_id: Option<String>,
    /// Specific machine to detonate on.
    pub machine_id: Option<i32>,
    pub plugins: Option<Vec<String>>,
    pub excluded_plugins: Option<Vec<String>>,
    /// Sample file name inside the guest; defaults to the uploaded name.
    pub target_filename: Option<String>,
    /// JSON-encoded submission parameters (`SubmissionParams`).
    pub params: Option<serde_json::Value>,
}

impl ApiClient {
    pub async fn list_tasks(&self) -> Result<Vec<TaskInfo>> {
        let response = self.client.get(self.url("/v1/tasks")).send().await?;
        let response = self.check_response(response).await?;
        let page: PaginatedTasks = response.json().await?;
        Ok(page.items)
    }

    pub async fn get_task(&self, task_id: i32) -> Result<TaskInfo> {
        let response = self
            .client
            .get(self.url(&format!("/v1/tasks/{}", task_id)))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn cancel_task(&self, task_id: i32) -> Result<()> {
        let response = self
            .client
            .post(self.url(&format!("/v1/tasks/{}/cancel", task_id)))
            .send()
            .await?;
        self.check_response(response).await?;
        Ok(())
    }

    pub async fn get_task_tree(&self, task_id: i32) -> Result<TaskTree> {
        let response = self
            .client
            .get(self.url(&format!("/v1/tasks/{}/tree", task_id)))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn get_artifact_content(&self, task_id: i32, artifact_id: i64) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(self.url(&format!(
                "/v1/tasks/{}/artifacts/{}/content",
                task_id, artifact_id
            )))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.bytes().await?.to_vec())
    }

    pub async fn submit_task(&self, request: SubmitTaskRequest) -> Result<TaskResponse> {
        let file_bytes = tokio::fs::read(&request.file_path)
            .await
            .map_err(crate::error::CliError::Io)?;

        let file_name = Path::new(&request.file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "data.bin".to_string());

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
