use crate::error::{RegistryError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Contents of `plugins.lock`: every plugin installed through the registry tooling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    /// Lockfile format version.
    pub schema_version: u32,
    /// Installed plugins keyed by plugin name.
    pub plugins: HashMap<String, LockedPlugin>,
}

/// Lockfile record describing how and from where one plugin was installed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPlugin {
    /// Installed version, branch name or commit, depending on `pin`.
    pub version: String,
    /// GitHub `owner/repo`; empty for local installs.
    pub repository: String,
    /// How the plugin was located.
    pub source: InstallSource,
    /// Whether a prebuilt asset or a source build was installed.
    pub install_method: InstallMethod,
    /// Name of the downloaded release asset for prebuilt installs.
    pub asset: Option<String>,
    /// Checksum of the downloaded asset, when recorded.
    pub checksum: Option<String>,
    /// RFC 3339 timestamp of the installation.
    pub installed_at: String,
    /// Ref the install tracks; drives update behaviour.
    #[serde(default)]
    pub pin: PinKind,
    /// Commit SHA built for source installs.
    #[serde(default)]
    pub commit: Option<String>,
    /// Source directory for local installs.
    #[serde(default)]
    pub path: Option<String>,
}

/// Where an installed plugin came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox CLI plugin list/info output"
)]
pub enum InstallSource {
    /// Looked up by name in the plugin registry.
    Registry,
    /// Installed directly from a GitHub `owner/repo`.
    Direct,
    /// Installed from a local directory.
    Local,
}

/// Whether the plugin was installed from a prebuilt asset or built from source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum InstallMethod {
    /// Downloaded a prebuilt release asset.
    Prebuilt,
    /// Built from a source checkout or local directory.
    Source,
}

/// What the install tracks. Orthogonal to `InstallMethod` (prebuilt vs
/// source): a `Release` pin can be either, a `Branch` or `Commit` pin is
/// always source. Drives update behaviour.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
#[non_exhaustive]
pub enum PinKind {
    /// Tracks the latest release.
    #[default]
    Release,
    /// Tracks the head of a branch.
    Branch {
        /// Tracked branch name.
        name: String,
    },
    /// Pinned to an exact commit.
    Commit,
    /// Installed from a local directory; never updated.
    Local,
}

impl PinKind {
    /// Human label for a tracked ref given the recorded version string:
    /// Release -> "v1.2.3", Branch -> "branch main", Commit -> "commit 1a2b3c4d5e6f".
    #[must_use]
    pub fn label(&self, version: &str) -> String {
        match self {
            PinKind::Release => format!("v{version}"),
            PinKind::Branch { name } => format!("branch {name}"),
            PinKind::Commit => format!("commit {}", short_commit(version)),
            PinKind::Local => "local".to_string(),
        }
    }
}

/// Display-friendly short commit: first 12 chars of a SHA, or the value as-is
/// if shorter / not a full SHA.
fn short_commit(sha: &str) -> &str {
    sha.get(..12).unwrap_or(sha)
}

impl Lockfile {
    /// Lockfile with the current schema version and no plugins.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            schema_version: 2,
            plugins: HashMap::new(),
        }
    }

    /// Load the lockfile at `path`, or an empty one if the file does not exist.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Io`] if the file cannot be read, or
    /// [`RegistryError::Lockfile`] if it is not valid lockfile JSON.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let contents = std::fs::read_to_string(path)?;
        serde_json::from_str(&contents).map_err(|e| {
            RegistryError::Lockfile(format!("failed to parse {}: {e}", path.display()))
        })
    }

    /// Atomically write the lockfile to `path` via a temporary file.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Json`] if serialization fails, [`RegistryError::Io`]
    /// if the directory or temp file cannot be written, or [`RegistryError::Lockfile`]
    /// if the temp file cannot be persisted over `path`.
    pub fn write(&self, path: &Path) -> Result<()> {
        let contents = serde_json::to_string_pretty(self)?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut tmp = tempfile::NamedTempFile::new_in(path.parent().unwrap_or(Path::new(".")))?;
        std::io::Write::write_all(&mut tmp, contents.as_bytes())?;
        tmp.persist(path)
            .map_err(|e| RegistryError::Lockfile(format!("failed to persist lockfile: {e}")))?;

        Ok(())
    }

    /// Lockfile location for a plugins directory: `plugins.lock` in its parent.
    #[must_use]
    pub fn lockfile_path(plugins_dir: &Path) -> std::path::PathBuf {
        plugins_dir
            .parent()
            .unwrap_or(plugins_dir)
            .join("plugins.lock")
    }

    /// Sorted names of subdirectories of `plugins_dir` with no lockfile entry.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::Io`] if the directory cannot be listed.
    pub fn untracked_plugins(&self, plugins_dir: &Path) -> Result<Vec<String>> {
        let mut untracked = Vec::new();

        if !plugins_dir.exists() {
            return Ok(untracked);
        }

        for entry in std::fs::read_dir(plugins_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !self.plugins.contains_key(&name) {
                    untracked.push(name);
                }
            }
        }

        untracked.sort();
        Ok(untracked)
    }
}
