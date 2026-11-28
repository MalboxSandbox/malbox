//! Plugin listing endpoints.

use super::ApiClient;
use crate::error::Result;
use serde::Deserialize;

/// A plugin known to the daemon, as returned by `GET /v1/plugins`.
#[derive(Deserialize, Debug)]
pub struct PluginInfo {
    /// Unique plugin name.
    pub name: String,
    /// Plugin version string.
    pub version: String,
    /// Optional human-readable description from the manifest.
    pub description: Option<String>,
    /// Plugin category (for example analyzer or provider).
    pub plugin_type: String,
    /// Lifecycle state reported by the plugin manager.
    pub state: String,
    /// How the plugin is executed (host process, guest, ...).
    pub execution: String,
    /// Filesystem path of the plugin executable.
    pub binary_path: String,
    /// Directory the plugin is installed in.
    pub plugin_dir: String,
    /// Current runtime status summary.
    pub status: String,
}

impl ApiClient {
    /// Lists installed plugins, optionally filtered by plugin type.
    ///
    /// # Errors
    ///
    /// Returns [`CliError::Http`](crate::error::CliError::Http) if the request fails or the
    /// response cannot be decoded, and [`CliError::Api`](crate::error::CliError::Api) if
    /// the daemon answers with an error status.
    pub async fn list_plugins(&self, plugin_type: Option<&str>) -> Result<Vec<PluginInfo>> {
        let mut url = self.url("/v1/plugins");
        if let Some(t) = plugin_type {
            url = format!("{url}?type={t}");
        }
        let response = self.client.get(&url).send().await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }
}
