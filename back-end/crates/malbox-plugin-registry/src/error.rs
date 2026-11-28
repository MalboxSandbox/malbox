//! Registry error type and plugin name validation.

use std::path::PathBuf;
use thiserror::Error;

/// Failure while resolving, fetching, installing or managing a plugin.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum RegistryError {
    /// No registry entry exists with the given plugin name.
    #[error("plugin '{0}' not found in registry")]
    PluginNotFound(String),

    /// The release has no prebuilt asset for the current platform.
    #[error("no matching asset for plugin '{plugin}' on platform {platform}")]
    NoPlatformAsset {
        /// Plugin whose release was searched.
        plugin: String,
        /// Platform triple that had no asset.
        platform: String,
    },

    /// The plugin directory already exists and `--force` was not given.
    #[error("plugin '{0}' is already installed (use --force to overwrite)")]
    AlreadyInstalled(String),

    /// The named plugin is not present in the lockfile or plugins directory.
    #[error("plugin '{0}' is not installed")]
    NotInstalled(String),

    /// A fetched or local plugin directory is malformed.
    #[error("invalid plugin at {path}: {reason}")]
    InvalidPlugin {
        /// Directory that failed validation.
        path: PathBuf,
        /// Human-readable cause.
        reason: String,
    },

    /// The install specifier or plugin name could not be parsed.
    #[error(
        "invalid specifier '{0}' — expected 'name', 'name@version', 'owner/repo', or 'owner/repo@version'"
    )]
    InvalidSpecifier(String),

    /// Mutually exclusive version/branch/commit selectors were combined.
    #[error("conflicting version selectors: {0}")]
    ConflictingSelectors(String),

    /// The lockfile could not be parsed or serialized.
    #[error("lockfile error: {0}")]
    Lockfile(String),

    /// GitHub or the registry returned an error or unexpected data.
    #[error("github error: {0}")]
    GitHub(String),

    /// A downloaded asset did not match its published checksum.
    #[error("checksum mismatch for asset '{0}'")]
    ChecksumMismatch(String),

    /// Building the plugin from source failed.
    #[error("build failed for plugin '{plugin}': {reason}")]
    BuildFailed {
        /// Plugin being built.
        plugin: String,
        /// Build tool output or failure description.
        reason: String,
    },

    /// An external tool required for the operation is not on `PATH`.
    #[error("required tool '{0}' not found in PATH")]
    ToolNotFound(String),

    /// HTTP request failed.
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Filesystem operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failed.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// The plugin's `plugin.toml` failed to load or validate.
    #[error("manifest error: {0}")]
    Manifest(#[from] malbox_plugin_manifest::ManifestError),

    /// Shared installer helpers (download, extract, checksum) failed.
    #[error("installer error: {0}")]
    Installer(#[from] malbox_installer::InstallError),
}

/// Result alias using [`RegistryError`].
pub type Result<T> = std::result::Result<T, RegistryError>;

/// Reject plugin names that are empty, contain path components, or use characters
/// outside alphanumerics, `-` and `_`.
///
/// # Errors
///
/// Returns [`RegistryError::InvalidSpecifier`] if `name` is not a safe plugin name.
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
