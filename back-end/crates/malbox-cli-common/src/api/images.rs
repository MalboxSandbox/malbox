//! Guest image registration and lookup endpoints.

use super::ApiClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Body of `POST /v1/images` registering a disk image with the daemon.
#[derive(Serialize, Debug)]
pub struct RegisterImageRequest {
    /// Unique image name.
    pub name: String,
    /// Guest platform, e.g. `windows` or `linux`.
    pub platform: String,
    /// CPU architecture of the guest.
    pub arch: String,
    /// Disk image format (e.g. `qcow2`); inferred by the daemon when absent.
    pub format: Option<String>,
    /// Optional free-text description.
    pub description: Option<String>,
    /// Path to the image file on the daemon host.
    pub path: String,
}

/// A registered guest image as returned by the daemon.
#[derive(Deserialize, Debug)]
pub struct Image {
    /// Database identifier, if assigned.
    pub id: Option<serde_json::Value>,
    /// Unique image name.
    pub name: String,
    /// Guest platform.
    pub platform: Option<serde_json::Value>,
    /// CPU architecture of the guest.
    pub arch: Option<serde_json::Value>,
    /// Disk image format.
    pub format: Option<String>,
    /// Optional free-text description.
    pub description: Option<String>,
    /// Path to the image file on the daemon host.
    pub path: Option<String>,
    /// Whether the image file currently exists on disk.
    pub available: Option<bool>,
    /// Registration timestamp.
    pub created_at: Option<serde_json::Value>,
    /// Last modification timestamp.
    pub updated_at: Option<serde_json::Value>,
}

impl ApiClient {
    /// Registers a new guest image and returns the stored record.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn register_image(&self, request: RegisterImageRequest) -> Result<Image> {
        let response = self
            .client
            .post(self.url("/v1/images"))
            .json(&request)
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Lists all registered guest images.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn list_images(&self) -> Result<Vec<Image>> {
        let response = self.client.get(self.url("/v1/images")).send().await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Fetches a single guest image by name.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn get_image(&self, name: &str) -> Result<Image> {
        let response = self
            .client
            .get(self.url(&format!("/v1/images/{name}")))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    /// Unregisters the guest image with the given name.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn delete_image(&self, name: &str) -> Result<()> {
        let response = self
            .client
            .delete(self.url(&format!("/v1/images/{name}")))
            .send()
            .await?;
        self.check_response(response).await?;
        Ok(())
    }
}
