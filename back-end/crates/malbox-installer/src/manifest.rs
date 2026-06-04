use crate::error::{InstallError, Step};
use crate::github::Channel;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub schema_version: u32,
    pub installed_at: String,
    pub updated_at: String,
    pub arch: String,
    /// Release channel upgrades follow. Manifests written before channels
    /// existed default to nightly, matching the previous upgrade behavior.
    #[serde(default)]
    pub channel: Channel,
    pub daemon: DaemonManifest,
    pub frontend: FrontendManifest,
    pub postgres: PostgresManifest,
    pub systemd: SystemdManifest,
    pub cli: CliManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_completed_step: Option<Step>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonManifest {
    pub source: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_path: Option<PathBuf>,
    /// Version the `prev_path` backup was taken from, restored on rollback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_version: Option<String>,
    pub providers: Vec<String>,
    #[serde(default)]
    pub provisioners: Vec<String>,
    /// Cargo features the installed binaries were built with. Replayed
    /// verbatim when an upgrade has to compile from source.
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendManifest {
    pub source: String,
    pub path: PathBuf,
    /// Previous front-end bundle kept for rollback (`web.prev`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresManifest {
    pub strategy: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdManifest {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliManifest {
    pub path: PathBuf,
}

impl Manifest {
    pub fn load(path: &Path) -> crate::Result<Self> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(InstallError::ManifestNotFound(path.to_path_buf()));
            }
            Err(e) => return Err(e.into()),
        };
        serde_json::from_str(&content).map_err(|e| InstallError::Manifest(e.to_string()))
    }

    pub fn save(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn mark_step_completed(&mut self, step: Step) {
        self.last_completed_step = Some(step);
    }

    pub fn mark_complete(&mut self) {
        self.last_completed_step = None;
    }

    pub fn record_upgrade(&mut self, new_version: &str, commit: Option<&str>) {
        self.version = new_version.to_string();
        self.daemon.version = new_version.to_string();
        self.daemon.commit = commit.map(String::from);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
            .unwrap_or_else(|| PathBuf::from("/etc"))
            .join("malbox")
            .join("manifest.json")
    }
}
