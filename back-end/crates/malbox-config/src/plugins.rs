use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Where plugins live on disk and how they are discovered and fetched.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginsConfig {
    #[serde(default = "default_directory")]
    /// Directory scanned for installed plugins.
    pub directory: PathBuf,

    #[serde(default = "default_watch")]
    /// Reload plugins when the directory changes.
    pub watch: bool,

    #[serde(default)]
    /// Remote plugin registry; `None` disables registry installs.
    pub registry: Option<RegistryConfig>,
}

/// Remote plugin registry location and local cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryConfig {
    #[serde(default = "default_registry_repo")]
    /// GitHub `owner/repo` hosting the registry index.
    pub repository: String,

    #[serde(default = "default_cache_dir")]
    /// Local directory caching downloaded registry data.
    pub cache_dir: PathBuf,
}

fn default_directory() -> PathBuf {
    directories::ProjectDirs::from("org", "malbox", "malbox").map_or_else(
        || PathBuf::from("/var/lib/malbox/plugins"),
        |dirs| dirs.config_dir().join("plugins"),
    )
}

fn default_watch() -> bool {
    true
}

fn default_registry_repo() -> String {
    "MalboxSandbox/malbox-plugin-registry".to_string()
}

fn default_cache_dir() -> PathBuf {
    directories::ProjectDirs::from("org", "malbox", "malbox").map_or_else(
        || PathBuf::from("/var/cache/malbox/registry"),
        |dirs| dirs.cache_dir().join("registry"),
    )
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            directory: default_directory(),
            watch: true,
            registry: None,
        }
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            repository: default_registry_repo(),
            cache_dir: default_cache_dir(),
        }
    }
}
