use crate::error::InstallError;
use serde::{Deserialize, Serialize};

const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

impl Release {
    pub fn version_from_tag(tag: &str) -> &str {
        tag.strip_prefix('v').unwrap_or(tag)
    }

    pub fn version(&self) -> &str {
        Self::version_from_tag(&self.tag_name)
    }

    pub fn find_daemon_asset(&self, arch: &str, providers: &[&str]) -> Option<&ReleaseAsset> {
        let mut sorted_providers: Vec<&str> = providers.to_vec();
        sorted_providers.sort();
        let suffix = sorted_providers.join("-");
        let expected = format!("malbox-daemon-{}-{}.tar.gz", arch, suffix);

        self.assets.iter().find(|a| a.name == expected)
    }

    pub fn find_frontend_asset(&self) -> Option<&ReleaseAsset> {
        self.assets
            .iter()
            .find(|a| a.name == "malbox-frontend.tar.gz")
    }

    pub fn find_source_asset(&self) -> Option<&ReleaseAsset> {
        self.assets.iter().find(|a| a.name == "source.tar.gz")
    }
}

pub struct GitHubClient {
    client: reqwest::Client,
    owner: String,
    repo: String,
}

impl GitHubClient {
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("malbox-installer")
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            owner: owner.into(),
            repo: repo.into(),
        }
    }

    pub async fn latest_release(&self) -> crate::Result<Release> {
        let url = format!(
            "{}/repos/{}/{}/releases/latest",
            GITHUB_API_BASE, self.owner, self.repo
        );

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(InstallError::GitHub(format!(
                "failed to fetch latest release: HTTP {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| InstallError::GitHub(e.to_string()))
    }

    pub async fn download_asset(&self, url: &str) -> crate::Result<Vec<u8>> {
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(InstallError::GitHub(format!(
                "failed to download asset: HTTP {}",
                response.status()
            )));
        }

        Ok(response.bytes().await?.to_vec())
    }
}
