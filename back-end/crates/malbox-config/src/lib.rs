use std::path::PathBuf;
use tokio::sync::OnceCell;
use tracing::debug;

pub mod cli;
pub mod core;
pub mod error;
pub mod guest_access;
pub mod images;
pub mod machinery;
pub mod machines;
pub mod plugins;
pub mod providers;
pub mod provisioning;
pub mod storage;
pub mod types;

pub use cli::CliConfig;
pub use core::Config;
pub use error::ConfigError;
pub use guest_access::{GuestAccessConfig, TransportKind};
pub use images::ImagesConfig;
pub use machinery::MachineryConfig;
pub use machines::{Arch, MachineConfig, validate_machine_configs};
pub use plugins::PluginsConfig;
pub use providers::ProvidersConfig;
pub use provisioning::ProvisionStep;
pub use storage::PathConfig;
pub use types::*;

pub static CONFIG: OnceCell<Config> = OnceCell::const_new();

pub async fn load_config() -> Result<&'static Config, ConfigError> {
    CONFIG
        .get_or_try_init(|| async { load_config_internal().await })
        .await
}

async fn load_config_internal() -> Result<Config, ConfigError> {
    let paths = PathConfig::new()?;

    // An explicit override (also set by the installer's systemd unit) takes
    // precedence over discovery. A broken override fails loudly instead of
    // silently falling back to another config.
    let config_path = if let Some(path) = env_config_override() {
        if !path.exists() {
            return Err(ConfigError::PathError {
                message: "MALBOX_CONFIG points to a file that does not exist".to_string(),
                path,
            });
        }
        debug!(path = %path.display(), "Loaded config from MALBOX_CONFIG");
        path
    } else if let Some(path) = find_user_config(&paths) {
        debug!(path = %path.display(), "Loaded user config");
        path
    } else if let Some(path) = find_system_config() {
        debug!(path = %path.display(), "Loaded system config");
        path
    } else {
        return Err(ConfigError::NotFound);
    };

    let content =
        tokio::fs::read_to_string(&config_path)
            .await
            .map_err(|e| ConfigError::Parse {
                file: config_path.display().to_string(),
                error: e.to_string(),
            })?;

    let mut config: Config = toml::from_str(&content).map_err(|e| ConfigError::Parse {
        file: config_path.display().to_string(),
        error: e.to_string(),
    })?;

    config.paths = paths;

    config.paths.ensure_dirs_exist().await?;
    debug!("Using paths: {:#?}", config.paths);

    Ok(config)
}

/// The configuration file `load_config` would read, following the same
/// precedence (`MALBOX_CONFIG` override, then user, then system config).
/// `None` when no configuration exists. The override is returned without an
/// existence check so a broken override fails loudly downstream instead of
/// silently editing another file.
pub fn config_file_path() -> Option<PathBuf> {
    if let Some(path) = env_config_override() {
        return Some(path);
    }
    let paths = PathConfig::new().ok()?;
    find_user_config(&paths).or_else(find_system_config)
}

fn env_config_override() -> Option<PathBuf> {
    std::env::var_os("MALBOX_CONFIG")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn find_user_config(paths: &PathConfig) -> Option<PathBuf> {
    let user_config = paths.config_dir.join("malbox.toml");
    if user_config.exists() {
        Some(user_config)
    } else {
        None
    }
}

fn find_system_config() -> Option<PathBuf> {
    let system_config = PathBuf::from("/etc/malbox/malbox.toml");
    if system_config.exists() {
        Some(system_config)
    } else {
        None
    }
}
