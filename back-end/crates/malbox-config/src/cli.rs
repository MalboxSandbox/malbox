use crate::error::ConfigError;
use crate::storage::PathConfig;
use serde::{Deserialize, Serialize};

/// Configuration for the `malbox` CLI client, read from `cli.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliConfig {
    /// How the CLI reaches the daemon's HTTP API.
    pub api: ApiConfig,
}

/// Connection settings for the daemon HTTP API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    /// Base URL of the daemon API, e.g. `http://127.0.0.1:8080`.
    #[serde(default = "default_api_url")]
    pub url: String,
}

fn default_api_url() -> String {
    "http://127.0.0.1:8080".to_string()
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            api: ApiConfig {
                url: default_api_url(),
            },
        }
    }
}

/// Load the CLI config from `~/.config/malbox/cli.toml`.
/// Returns default config if file does not exist.
///
/// # Errors
///
/// Returns [`ConfigError::PathError`] when XDG directories cannot be determined and
/// [`ConfigError::Parse`] when the file exists but cannot be read or parsed.
pub fn load_cli_config() -> Result<CliConfig, ConfigError> {
    let paths = PathConfig::new()?;
    let config_path = paths.config_dir.join("cli.toml");

    if !config_path.exists() {
        return Ok(CliConfig::default());
    }

    let content = std::fs::read_to_string(&config_path).map_err(|e| ConfigError::Parse {
        file: config_path.display().to_string(),
        error: e.to_string(),
    })?;

    toml::from_str(&content).map_err(|e| ConfigError::Parse {
        file: config_path.display().to_string(),
        error: e.to_string(),
    })
}
