use super::ApiClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct RegisterImageRequest {
    pub name: String,
    pub platform: String,
    pub arch: String,
    pub format: Option<String>,
    pub description: Option<String>,
    pub path: String,
}

#[derive(Deserialize, Debug)]
pub struct Image {
    pub id: Option<serde_json::Value>,
    pub name: String,
    pub platform: Option<serde_json::Value>,
    pub arch: Option<serde_json::Value>,
    pub format: Option<String>,
    pub description: Option<String>,
    pub path: Option<String>,
    pub available: Option<bool>,
    pub created_at: Option<serde_json::Value>,
    pub updated_at: Option<serde_json::Value>,
}

impl ApiClient {
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

    pub async fn list_images(&self) -> Result<Vec<Image>> {
        let response = self.client.get(self.url("/v1/images")).send().await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn get_image(&self, name: &str) -> Result<Image> {
        let response = self
            .client
            .get(self.url(&format!("/v1/images/{}", name)))
            .send()
            .await?;
        let response = self.check_response(response).await?;
        Ok(response.json().await?)
    }

    pub async fn delete_image(&self, name: &str) -> Result<()> {
        let response = self
            .client
            .delete(self.url(&format!("/v1/images/{}", name)))
            .send()
            .await?;
        self.check_response(response).await?;
        Ok(())
    }
}
