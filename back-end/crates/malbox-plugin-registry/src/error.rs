use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("plugin '{0}' not found in registry")]
    PluginNotFound(String),

    #[error("no matching asset for plugin '{plugin}' on platform {platform}")]
    NoPlatformAsset { plugin: String, platform: String },

    #[error("plugin '{0}' is already installed (use --force to overwrite)")]
    AlreadyInstalled(String),

    #[error("plugin '{0}' is not installed")]
    NotInstalled(String),

    #[error("invalid plugin at {path}: {reason}")]
    InvalidPlugin { path: PathBuf, reason: String },

    #[error(
        "invalid specifier '{0}' — expected 'name', 'name@version', 'owner/repo', or 'owner/repo@version'"
    )]
    InvalidSpecifier(String),

    #[error("conflicting version selectors: {0}")]
    ConflictingSelectors(String),

    #[error("lockfile error: {0}")]
    Lockfile(String),

    #[error("github error: {0}")]
    GitHub(String),

    #[error("checksum mismatch for asset '{0}'")]
    ChecksumMismatch(String),

    #[error("build failed for plugin '{plugin}': {reason}")]
    BuildFailed { plugin: String, reason: String },

    #[error("required tool '{0}' not found in PATH")]
    ToolNotFound(String),

    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("manifest error: {0}")]
    Manifest(#[from] malbox_plugin_manifest::ManifestError),

    #[error("installer error: {0}")]
    Installer(#[from] malbox_installer::InstallError),
}

pub type Result<T> = std::result::Result<T, RegistryError>;

pub fn validate_plugin_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.')
        || !name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(RegistryError::InvalidSpecifier(format!(
            "invalid plugin name '{name}': must be alphanumeric with '-' or '_'"
        )));
    }
    Ok(())
}
