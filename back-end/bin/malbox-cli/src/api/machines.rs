use super::ApiClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct CreateMachineRequest {
    pub name: String,
    pub image: String,
    pub platform: String,
    pub arch: String,
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub struct Machine {
    pub id: Option<i32>,
    pub name: String,
    pub label: Option<String>,
    pub arch: serde_json::Value,
    pub platform: serde_json::Value,
    pub ip: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: serde_json::Value,
    pub image_id: Option<serde_json::Value>,
    pub provider: Option<String>,
    pub current_task_id: Option<i32>,
    pub error_message: Option<String>,
    pub created_at: Option<serde_json::Value>,
    pub updated_at: Option<serde_json::Value>,
}

impl ApiClient {
    pub async fn create_machine(&self, request: CreateMachineRequest) -> Result<Machine> {
        let response = self
            .client
            .post(self.url("/v1/machines"))
            .json(&request)
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn list_machines(&self) -> Result<Vec<Machine>> {
        let response = self.client.get(self.url("/v1/machines")).send().await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn get_machine(&self, id: i32) -> Result<Machine> {
        let response = self
            .client
            .get(self.url(&format!("/v1/machines/{}", id)))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn delete_machine(&self, id: i32) -> Result<()> {
        let response = self
            .client
            .delete(self.url(&format!("/v1/machines/{}", id)))
            .send()
            .await?;
        self.check_response(response).await?;
        Ok(())
    }

    pub async fn retry_machine(&self, id: i32) -> Result<Machine> {
        let response = self
            .client
            .post(self.url(&format!("/v1/machines/{}/retry", id)))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }
}
