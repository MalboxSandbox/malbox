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
    pub file_path: String,
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

pub struct SubmitTaskRequest {
    pub file_path: String,
    pub package: Option<String>,
    pub module: Option<String>,
    pub timeout: Option<i64>,
    pub priority: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub owner: Option<String>,
    pub memory: bool,
    pub unique: bool,
    pub enforce_timeout: bool,
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

        if let Some(package) = request.package {
            form = form.text("package", package);
        }
        if let Some(module) = request.module {
            form = form.text("module", module);
        }
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
        if request.memory {
            form = form.text("memory", "true");
        }
        if request.unique {
            form = form.text("unique", "true");
        }
        if request.enforce_timeout {
            form = form.text("enforce_timeout", "true");
        }

        let response = self
            .client
            .post(self.url("/v1/tasks/create/file"))
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
