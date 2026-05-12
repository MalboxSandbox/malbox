use super::ApiClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

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
    pub provider_id: Option<String>,
    pub error_message: Option<String>,
    pub created_at: Option<serde_json::Value>,
    pub updated_at: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
pub struct MachineSnapshot {
    pub id: serde_json::Value,
    pub machine_id: i32,
    pub name: String,
    pub provider_snapshot_id: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub is_active: bool,
    pub created_at: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ProvisionRequest {
    pub provisioner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revert_to: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ProvisionRunResponse {
    pub id: serde_json::Value,
    pub machine_id: i32,
    pub provisioner: String,
    pub status: String,
    pub config: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub snapshot_id: Option<serde_json::Value>,
    pub created_at: Option<serde_json::Value>,
    pub updated_at: Option<serde_json::Value>,
}

pub type ProvisionResponse = ProvisionRunResponse;

impl ApiClient {
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

    pub async fn resolve_machine(&self, name_or_id: &str) -> Result<Machine> {
        if let Ok(id) = name_or_id.parse::<i32>()
            && let Ok(m) = self.get_machine(id).await
        {
            return Ok(m);
        }
        let machines = self.list_machines().await?;
        machines
            .into_iter()
            .find(|m| m.name.eq_ignore_ascii_case(name_or_id))
            .ok_or_else(|| {
                crate::error::CliError::InvalidArgument(format!(
                    "machine '{}' not found",
                    name_or_id
                ))
            })
    }

    pub async fn resolve_machine_id(&self, name_or_id: &str) -> Result<i32> {
        let machine = self.resolve_machine(name_or_id).await?;
        machine
            .id
            .ok_or_else(|| crate::error::CliError::InvalidArgument("machine has no ID".to_string()))
    }

    pub async fn list_snapshots(&self, machine_id: i32) -> Result<Vec<MachineSnapshot>> {
        let response = self
            .client
            .get(self.url(&format!("/v1/machines/{}/snapshots", machine_id)))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn provision_machine(
        &self,
        machine_id: i32,
        request: ProvisionRequest,
    ) -> Result<ProvisionResponse> {
        let response = self
            .client
            .post(self.url(&format!("/v1/machines/{}/provision", machine_id)))
            .json(&request)
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn delete_snapshot(&self, machine_id: i32, snapshot_name: &str) -> Result<()> {
        let response = self
            .client
            .delete(self.url(&format!(
                "/v1/machines/{}/snapshots/{}",
                machine_id, snapshot_name
            )))
            .send()
            .await?;
        self.check_response(response).await?;
        Ok(())
    }

    pub async fn list_provision_runs(&self, machine_id: i32) -> Result<Vec<ProvisionRunResponse>> {
        let response = self
            .client
            .get(self.url(&format!("/v1/machines/{}/provisions", machine_id)))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }
}
