use crate::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// XDG-derived directories malbox reads configuration from and stores data in.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathConfig {
    /// Directory holding configuration files.
    #[serde(default = "default_config_dir")]
    pub config_dir: PathBuf,
    /// Directory for regenerable cached data.
    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
    /// Directory for persistent application data.
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    /// Directory for runtime state that should survive restarts.
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
}

impl Default for PathConfig {
    fn default() -> Self {
        Self {
            config_dir: default_config_dir(),
            cache_dir: default_cache_dir(),
            data_dir: default_data_dir(),
            state_dir: default_state_dir(),
        }
    }
}

// NOTE: Should probably be handled somewhere else, not malbox-config
impl PathConfig {
    /// Resolves the per-user XDG directories for malbox.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::PathError`] when the home directory or the XDG state
    /// directory cannot be determined.
    pub fn new() -> Result<Self, ConfigError> {
        let proj_dirs =
            directories::ProjectDirs::from("org", "malbox", "malbox").ok_or_else(|| {
                ConfigError::PathError {
                    message: "Failed to determine XDG directories".into(),
                    path: PathBuf::new(),
                }
            })?;
        let state_dir = proj_dirs
            .state_dir()
            .ok_or_else(|| ConfigError::PathError {
                message: "Failed to determine XDG state directory".into(),
                path: PathBuf::new(),
            })?
            .to_path_buf();
        Ok(Self {
            config_dir: proj_dirs.config_dir().to_path_buf(),
            cache_dir: proj_dirs.cache_dir().to_path_buf(),
            data_dir: proj_dirs.data_dir().to_path_buf(),
            state_dir,
        })
    }

    /// Creates every configured directory, including missing parents.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::PathError`] naming the first directory that could not be created.
    pub async fn ensure_dirs_exist(&self) -> Result<(), ConfigError> {
        for dir in [
            &self.config_dir,
            &self.cache_dir,
            &self.data_dir,
            &self.state_dir,
        ] {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| ConfigError::PathError {
                    message: e.to_string(),
                    path: dir.clone(),
                })?;
        }
        Ok(())
    }
}

fn default_config_dir() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "malbox", "malbox") {
        proj_dirs.config_dir().to_path_buf()
    } else {
        PathBuf::from("/etc/malbox")
    }
}

fn default_cache_dir() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "malbox", "malbox") {
        proj_dirs.cache_dir().to_path_buf()
    } else {
        PathBuf::from("/var/cache/malbox")
    }
}

fn default_data_dir() -> PathBuf {
    if let Some(proj_dirs) = directories::ProjectDirs::from("org", "malbox", "malbox") {
        proj_dirs.data_dir().to_path_buf()
    } else {
        PathBuf::from("/var/lib/malbox")
    }
}

fn default_state_dir() -> PathBuf {
    directories::ProjectDirs::from("org", "malbox", "malbox")
        .and_then(|proj_dirs| proj_dirs.state_dir().map(std::path::Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("/var/lib/malbox/state"))
}
