use crate::error::{InstallError, Step};
use crate::github::Channel;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Record of an installation, persisted as JSON to drive upgrades, resume, and rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Installed malbox release version.
    pub version: String,
    /// Manifest format version, bumped on incompatible layout changes.
    pub schema_version: u32,
    /// RFC 3339 timestamp of the initial install.
    pub installed_at: String,
    /// RFC 3339 timestamp of the last install or upgrade.
    pub updated_at: String,
    /// Release architecture of the installed binaries (e.g. `x86_64`).
    pub arch: String,
    /// Release channel upgrades follow. Manifests written before channels
    /// existed default to nightly, matching the previous upgrade behavior.
    #[serde(default)]
    pub channel: Channel,
    /// Installed daemon binaries.
    pub daemon: DaemonManifest,
    /// Installed front-end bundle.
    pub frontend: FrontendManifest,
    /// Database used by the daemon.
    pub postgres: PostgresManifest,
    /// systemd user service state.
    pub systemd: SystemdManifest,
    /// Installed CLI binary.
    pub cli: CliManifest,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Last finished step of an interrupted install; `None` once complete.
    pub last_completed_step: Option<Step>,
}

/// Installed daemon binaries and how they were obtained.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonManifest {
    /// How the binaries were obtained (`prebuilt` or `compile`).
    pub source: String,
    /// Version of the installed daemon.
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Git commit the daemon was built from, when known.
    pub commit: Option<String>,
    /// Directory holding the daemon binaries.
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Backup of the previous binaries kept for rollback.
    pub prev_path: Option<PathBuf>,
    /// Version the `prev_path` backup was taken from, restored on rollback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_version: Option<String>,
    /// Virtualization providers enabled in the daemon config.
    pub providers: Vec<String>,
    #[serde(default)]
    /// Machine provisioners compiled into the daemon.
    pub provisioners: Vec<String>,
    /// Cargo features the installed binaries were built with. Replayed
    /// verbatim when an upgrade has to compile from source.
    #[serde(default)]
    pub features: Vec<String>,
}

/// Installed front-end bundle and its rollback backup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendManifest {
    /// How the bundle was obtained (`prebuilt` or `compile`).
    pub source: String,
    /// Directory the bundle is served from.
    pub path: PathBuf,
    /// Previous front-end bundle kept for rollback (`web.prev`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_path: Option<PathBuf>,
}

/// Database the daemon connects to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresManifest {
    /// `existing` for a user-supplied server or `setup` for a managed one.
    pub strategy: String,
    /// Connection URL of the database server.
    pub url: String,
}

/// systemd user service installation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemdManifest {
    /// Whether the daemon runs as a systemd user service.
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Name of the installed unit file.
    pub unit: Option<String>,
}

/// Installed `malbox` CLI binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliManifest {
    /// Path of the CLI executable.
    pub path: PathBuf,
}

impl Manifest {
    /// Reads a manifest from `path`.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::ManifestNotFound`] when the file is missing,
    /// [`InstallError::Io`] on other read failures, and [`InstallError::Manifest`]
    /// when the JSON is invalid.
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

    /// Writes the manifest as pretty JSON to `path`, creating parent directories.
    ///
    /// # Errors
    ///
    /// Returns [`InstallError::Io`] when directories or the file cannot be written and
    /// [`InstallError::Json`] when serialization fails.
    pub fn save(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Records `step` as the last finished step so an interrupted install can resume.
    pub fn mark_step_completed(&mut self, step: Step) {
        self.last_completed_step = Some(step);
    }

    /// Clears resume state once every install step has finished.
    pub fn mark_complete(&mut self) {
        self.last_completed_step = None;
    }

    /// Updates versions, commit, and timestamp after a successful upgrade.
    pub fn record_upgrade(&mut self, new_version: &str, commit: Option<&str>) {
        self.version = new_version.to_string();
        self.daemon.version = new_version.to_string();
        self.daemon.commit = commit.map(String::from);
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    /// Standard manifest location: `$XDG_CONFIG_HOME/malbox/manifest.json`.
    #[must_use]
    pub fn default_path() -> PathBuf {
        dirs::config_dir()
            .or_else(|| dirs::home_dir().map(|home| home.join(".config")))
            .unwrap_or_else(|| PathBuf::from("/etc"))
            .join("malbox")
            .join("manifest.json")
    }
}
