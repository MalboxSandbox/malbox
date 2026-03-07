use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Directory containing plugin subdirectories.
    #[serde(default = "default_directory")]
    pub directory: PathBuf,

    /// Whether to watch for filesystem changes at runtime (default: true).
    #[serde(default = "default_watch")]
    pub watch: bool,
}

fn default_directory() -> PathBuf {
    directories::ProjectDirs::from("org", "malbox", "malbox")
        .map(|dirs| dirs.config_dir().join("plugins"))
        .unwrap_or_else(|| PathBuf::from("/var/lib/malbox/plugins"))
}

fn default_watch() -> bool {
    true
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            directory: default_directory(),
            watch: true,
        }
    }
}
