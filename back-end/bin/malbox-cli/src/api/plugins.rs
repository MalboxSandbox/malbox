use super::ApiClient;
use crate::error::Result;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub plugin_type: String,
    pub state: String,
    pub execution: String,
    pub binary_path: String,
    pub plugin_dir: String,
    pub status: String,
}

impl ApiClient {
    pub async fn list_plugins(&self, plugin_type: Option<&str>) -> Result<Vec<PluginInfo>> {
        let mut url = self.url("/v1/plugins");
        if let Some(t) = plugin_type {
            url = format!("{}?type={}", url, t);
        }
        let response = self.client.get(&url).send().await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }
}
