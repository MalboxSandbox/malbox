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
pub struct TaskResultInfo {
    pub id: i32,
    pub task_id: i32,
    pub plugin_name: String,
    pub result_name: String,
    pub format: String,
    pub size_bytes: i64,
    pub created_on: String,
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
    pub plugins: Option<Vec<String>>,
    /// Sample file name inside the guest; defaults to the uploaded name.
    pub target_filename: Option<String>,
}

impl ApiClient {
    pub async fn list_tasks(&self) -> Result<Vec<TaskInfo>> {
        let response = self.client.get(self.url("/v1/tasks")).send().await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
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
        if let Some(plugins) = request.plugins {
            form = form.text("plugins", plugins.join(","));
        }
        if let Some(target_filename) = request.target_filename {
            form = form.text("target_filename", target_filename);
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

    pub async fn get_task_results(&self, task_id: i32) -> Result<Vec<TaskResultInfo>> {
        let response = self
            .client
            .get(self.url(&format!("/v1/tasks/{}/results", task_id)))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn get_task_result_content(&self, task_id: i32, result_id: i32) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(self.url(&format!(
                "/v1/tasks/{}/results/{}/content",
                task_id, result_id
            )))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.bytes().await?.to_vec())
    }
}
