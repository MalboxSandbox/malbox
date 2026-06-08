use crate::error::{RegistryError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lockfile {
    pub schema_version: u32,
    pub plugins: HashMap<String, LockedPlugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockedPlugin {
    pub version: String,
    pub repository: String,
    pub source: InstallSource,
    pub install_method: InstallMethod,
    pub asset: Option<String>,
    pub checksum: Option<String>,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallSource {
    Registry,
    Direct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMethod {
    Prebuilt,
    Source,
}

impl Lockfile {
    pub fn empty() -> Self {
        Self {
            schema_version: 1,
            plugins: HashMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::empty());
        }
        let contents = std::fs::read_to_string(path)?;
        serde_json::from_str(&contents).map_err(|e| {
            RegistryError::Lockfile(format!("failed to parse {}: {e}", path.display()))
        })
    }

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

    pub fn lockfile_path(plugins_dir: &Path) -> std::path::PathBuf {
        plugins_dir
            .parent()
            .unwrap_or(plugins_dir)
            .join("plugins.lock")
    }

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
