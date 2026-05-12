pub mod images;
pub mod machines;
pub mod plugins;
pub mod tasks;

use crate::error::{CliError, Result};
use reqwest::Client;
use serde::Deserialize;

pub struct ApiClient {
    client: Client,
    base_url: String,
}

/// Standard error body returned by the malbox API.
#[derive(Deserialize)]
struct ApiErrorBody {
    error: String,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Check a response for API-level errors (4xx/5xx with JSON body).
    /// Returns the response unchanged if status is success.
    async fn check_response(&self, response: reqwest::Response) -> Result<reqwest::Response> {
        if response.status().is_success() {
            return Ok(response);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if let Ok(err_body) = serde_json::from_str::<ApiErrorBody>(&body) {
            Err(CliError::Api(format!("{}: {}", status, err_body.error)))
        } else {
            Err(CliError::Api(format!("{}: {}", status, body)))
        }
    }
}
