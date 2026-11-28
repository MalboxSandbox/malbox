//! Analysis machine, snapshot, and provisioning endpoints.

use super::ApiClient;
use crate::error::{CliError, Result};
use serde::{Deserialize, Serialize};

/// An analysis machine managed by a provider.
#[derive(Deserialize, Debug)]
pub struct Machine {
    /// Database identifier, if assigned.
    pub id: Option<i32>,
    /// Unique machine name.
    pub name: String,
    /// Provider-side label of the machine.
    pub label: Option<String>,
    /// CPU architecture.
    pub arch: serde_json::Value,
    /// Guest platform.
    pub platform: serde_json::Value,
    /// Guest IP address, once known.
    pub ip: Option<String>,
    /// Free-form tags used for machine selection.
    pub tags: Option<Vec<String>>,
    /// Lifecycle status (idle, running, ...).
    pub status: serde_json::Value,
    /// Image the machine was created from.
    pub image_id: Option<serde_json::Value>,
    /// Name of the provider plugin that manages the machine.
    pub provider: Option<String>,
    /// Task currently running on the machine, if any.
    pub current_task_id: Option<i32>,
    /// Identifier of the machine inside its provider.
    pub provider_id: Option<String>,
    /// Last error reported for the machine.
    pub error_message: Option<String>,
    /// Creation timestamp.
    pub created_at: Option<serde_json::Value>,
    /// Last modification timestamp.
    pub updated_at: Option<serde_json::Value>,
}

/// A saved snapshot of a machine that tasks can revert to.
#[derive(Deserialize, Debug)]
pub struct MachineSnapshot {
    /// Snapshot UUID.
    pub id: serde_json::Value,
    /// Machine the snapshot belongs to.
    pub machine_id: i32,
    /// Snapshot name, unique per machine.
    pub name: String,
    /// Identifier of the snapshot inside the provider.
    pub provider_snapshot_id: String,
    /// Optional free-text description.
    pub description: Option<String>,
    /// Free-form tags used for snapshot selection.
    pub tags: Option<Vec<String>>,
    /// Whether this is the machine's default snapshot.
    pub is_active: bool,
    /// Creation timestamp.
    pub created_at: Option<serde_json::Value>,
}

/// Body of `POST /v1/machines/{id}/provision`.
#[derive(Serialize, Debug)]
pub struct ProvisionRequest {
    /// Provisioner plugin to run.
    pub provisioner: String,
    /// Provisioner-specific configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    /// Guest plugins to install during provisioning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<Vec<String>>,
    /// Name of the snapshot to take after provisioning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    /// Snapshot to revert to before provisioning.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revert_to: Option<String>,
}

/// A provisioning run of a machine and its outcome.
#[derive(Deserialize, Debug)]
pub struct ProvisionRunResponse {
    /// Run UUID.
    pub id: serde_json::Value,
    /// Machine being provisioned.
    pub machine_id: i32,
    /// Provisioner plugin that ran.
    pub provisioner: String,
    /// Run status (pending, running, completed, failed).
    pub status: String,
    /// Configuration the provisioner was given.
    pub config: Option<serde_json::Value>,
    /// Provisioner output, once finished.
    pub output: Option<serde_json::Value>,
    /// Failure reason, if the run failed.
    pub error_message: Option<String>,
    /// Snapshot created by the run, if any.
    pub snapshot_id: Option<serde_json::Value>,
    /// Creation timestamp.
    pub created_at: Option<serde_json::Value>,
    /// Last modification timestamp.
    pub updated_at: Option<serde_json::Value>,
}

/// Response of `POST /v1/machines/{id}/provision`; the newly started run.
pub type ProvisionResponse = ProvisionRunResponse;

impl ApiClient {
    /// Lists all machines known to the daemon.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn list_machines(&self) -> Result<Vec<Machine>> {
        let response = self.client.get(self.url("/v1/machines")).send().await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Fetches a machine by its database identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn get_machine(&self, id: i32) -> Result<Machine> {
        let response = self
            .client
            .get(self.url(&format!("/v1/machines/{id}")))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Finds a machine by numeric ID or, failing that, by case-insensitive name.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::InvalidArgument`] if no machine matches, and the
    /// errors of [`Self::list_machines`] if the machine list cannot be fetched.
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
            .ok_or_else(|| CliError::InvalidArgument(format!("machine '{name_or_id}' not found")))
    }

    /// Resolves a machine name or ID (see [`Self::resolve_machine`]) to its database ID.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`Self::resolve_machine`], or
    /// [`CliError::InvalidArgument`] if the matched machine has no ID.
    pub async fn resolve_machine_id(&self, name_or_id: &str) -> Result<i32> {
        let machine = self.resolve_machine(name_or_id).await?;
        machine
            .id
            .ok_or_else(|| CliError::InvalidArgument("machine has no ID".to_string()))
    }

    /// Lists the snapshots of a machine.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn list_snapshots(&self, machine_id: i32) -> Result<Vec<MachineSnapshot>> {
        let response = self
            .client
            .get(self.url(&format!("/v1/machines/{machine_id}/snapshots")))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Starts a provisioning run on a machine.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn provision_machine(
        &self,
        machine_id: i32,
        request: ProvisionRequest,
    ) -> Result<ProvisionResponse> {
        let response = self
            .client
            .post(self.url(&format!("/v1/machines/{machine_id}/provision")))
            .json(&request)
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Deletes a named snapshot of a machine.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn delete_snapshot(&self, machine_id: i32, snapshot_name: &str) -> Result<()> {
        let response = self
            .client
            .delete(self.url(&format!(
                "/v1/machines/{machine_id}/snapshots/{snapshot_name}"
            )))
            .send()
            .await?;
        self.check_response(response).await?;
        Ok(())
    }

    /// Lists past and current provisioning runs of a machine.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn list_provision_runs(&self, machine_id: i32) -> Result<Vec<ProvisionRunResponse>> {
        let response = self
            .client
            .get(self.url(&format!("/v1/machines/{machine_id}/provisions")))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }
}
