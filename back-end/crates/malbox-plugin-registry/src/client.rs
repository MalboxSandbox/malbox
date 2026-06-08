use crate::error::{RegistryError, Result};
use crate::index::{PluginMetadata, RegistryIndex};
use std::path::PathBuf;
use std::time::Duration;

const RAW_GITHUB_BASE: &str = "https://raw.githubusercontent.com";
const DEFAULT_BRANCH: &str = "main";

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn write(&self, key: &str, content: &[u8], etag: Option<&str>) -> Result<()> {
        let path = self.dir.join(key);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, content)?;

        let etag_path = self.etag_path(key);
        if let Some(etag) = etag {
            std::fs::write(&etag_path, etag)?;
        } else if etag_path.exists() {
            std::fs::remove_file(&etag_path)?;
        }
        Ok(())
    }

    pub fn read(&self, key: &str) -> Result<Option<(Vec<u8>, Option<String>)>> {
        let path = self.dir.join(key);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read(&path)?;
        let etag = self.read_etag(key);
        Ok(Some((content, etag)))
    }

    fn read_etag(&self, key: &str) -> Option<String> {
        let etag_path = self.etag_path(key);
        std::fs::read_to_string(etag_path).ok()
    }

    fn etag_path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.etag"))
    }
}

pub struct RegistryClient {
    http: reqwest::Client,
    owner: String,
    repo: String,
    cache: Cache,
}

impl RegistryClient {
    pub fn new(repository: &str, cache_dir: PathBuf) -> Result<Self> {
        let (owner, repo) = repository
            .split_once('/')
            .ok_or_else(|| RegistryError::GitHub(format!("invalid repository: {repository}")))?;

        let http = reqwest::Client::builder()
            .user_agent("malbox-plugin-registry")
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(30))
            .build()?;

        Ok(Self {
            http,
            owner: owner.to_string(),
            repo: repo.to_string(),
            cache: Cache::new(cache_dir),
        })
    }

    pub async fn fetch_index(&self) -> Result<RegistryIndex> {
        let bytes = self.fetch_cached("index.json").await?;
        serde_json::from_slice(&bytes)
            .map_err(|e| RegistryError::GitHub(format!("failed to parse registry index: {e}")))
    }

    pub async fn fetch_plugin_metadata(&self, name: &str) -> Result<PluginMetadata> {
        crate::error::validate_plugin_name(name)?;
        let key = format!("plugins/{name}.json");
        let bytes = self.fetch_cached(&key).await?;
        serde_json::from_slice(&bytes).map_err(|e| {
            RegistryError::GitHub(format!("failed to parse plugin metadata for '{name}': {e}"))
        })
    }

    async fn fetch_cached(&self, path: &str) -> Result<Vec<u8>> {
        let url = format!(
            "{RAW_GITHUB_BASE}/{}/{}/{DEFAULT_BRANCH}/{path}",
            self.owner, self.repo
        );

        let cached = self.cache.read(path)?;
        let mut request = self.http.get(&url);

        if let Some((_, Some(ref etag))) = cached {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }

        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            if !token.is_empty() {
                request = request.bearer_auth(token);
            }
        }

        let response = request.send().await?;
        let status = response.status();

        if status == reqwest::StatusCode::NOT_MODIFIED {
            if let Some((content, _)) = cached {
                return Ok(content);
            }
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(RegistryError::GitHub(format!(
                "registry file not found: {path}"
            )));
        }

        if !status.is_success() {
            return Err(RegistryError::GitHub(format!(
                "failed to fetch {path}: HTTP {status}"
            )));
        }

        let etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let bytes = response.bytes().await?.to_vec();
        self.cache.write(path, &bytes, etag.as_deref())?;

        Ok(bytes)
    }
}
