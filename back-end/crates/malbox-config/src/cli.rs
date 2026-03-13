use crate::storage::PathConfig;
use crate::error::ConfigError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
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
pub fn load_cli_config() -> Result<CliConfig, ConfigError> {
    let paths = PathConfig::new()?;
    let config_path = paths.config_dir.join("cli.toml");

    if !config_path.exists() {
        return Ok(CliConfig::default());
    }

    let content = std::fs::read_to_string(&config_path)
        .map_err(|e| ConfigError::Parse {
            file: config_path.display().to_string(),
            error: e.to_string(),
        })?;

    toml::from_str(&content).map_err(|e| ConfigError::Parse {
        file: config_path.display().to_string(),
        error: e.to_string(),
    })
}
