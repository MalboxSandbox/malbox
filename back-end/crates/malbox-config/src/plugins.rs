use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginsConfig {
    #[serde(default = "default_directory")]
    pub directory: PathBuf,

    #[serde(default = "default_watch")]
    pub watch: bool,

    #[serde(default)]
    pub registry: Option<RegistryConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryConfig {
    #[serde(default = "default_registry_repo")]
    pub repository: String,

    #[serde(default = "default_cache_dir")]
    pub cache_dir: PathBuf,
}

fn default_directory() -> PathBuf {
    directories::ProjectDirs::from("org", "malbox", "malbox")
        .map(|dirs| dirs.config_dir().join("plugins"))
        .unwrap_or_else(|| PathBuf::from("/var/lib/malbox/plugins"))
}

fn default_watch() -> bool {
    true
}

fn default_registry_repo() -> String {
    "DualHorizon/malbox-plugin-registry".to_string()
}

fn default_cache_dir() -> PathBuf {
    directories::ProjectDirs::from("org", "malbox", "malbox")
        .map(|dirs| dirs.cache_dir().join("registry"))
        .unwrap_or_else(|| PathBuf::from("/var/cache/malbox/registry"))
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
