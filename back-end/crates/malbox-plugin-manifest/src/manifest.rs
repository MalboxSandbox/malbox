use crate::error::ManifestError;
use crate::runtime::RuntimeConfig;

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Top-level manifest structure parsed from `plugin.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginInfo,
    #[serde(default)]
    pub scope: Option<ScopeConfig>,
    #[serde(default)]
    pub results: HashMap<String, ResultConfig>,
    #[serde(default)]
    pub events: Option<EventsConfig>,
    pub runtime: RuntimeConfig,
}

/// Core plugin metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(rename = "type")]
    pub plugin_type: PluginTypeConfig,
    /// Optional explicit binary name (defaults to directory name).
    #[serde(default)]
    pub binary: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginTypeConfig {
    Host,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginStateConfig {
    Persistent,
    Ephemeral,
    Scoped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionContextConfig {
    Exclusive,
    Sequential,
    Parallel,
    Unrestricted,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScopeConfig {
    #[serde(default)]
    pub plugins: Vec<String>,
    #[serde(default)]
    pub task_types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResultConfig {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub user_visible: Option<bool>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub render: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventsConfig {
    #[serde(default)]
    pub subscribe: Vec<String>,
    #[serde(default)]
    pub filters: HashMap<String, EventFilterConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventFilterConfig {
    #[serde(default)]
    pub from_plugins: Vec<String>,
}

/// Validate structural correctness of a parsed manifest: name/version/scope.
///
/// Called by `parse_manifest` after TOML deserialization succeeds.
pub fn validate_manifest(manifest: &PluginManifest) -> Result<(), ManifestError> {
    if manifest.plugin.name.is_empty() {
        return Err(ManifestError::Invalid(
            "plugin name must not be empty".into(),
        ));
    }

    if !manifest
        .plugin
        .name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ManifestError::Invalid(format!(
            "plugin name '{}' contains invalid characters (allowed: alphanumeric, '-', '_')",
            manifest.plugin.name
        )));
    }

    semver::Version::parse(&manifest.plugin.version).map_err(|e| {
        ManifestError::Invalid(format!(
            "invalid version '{}': {}",
            manifest.plugin.version, e
        ))
    })?;

    if manifest.runtime.state == PluginStateConfig::Scoped && manifest.scope.is_none() {
        return Err(ManifestError::Invalid(
            "plugin with state 'scoped' must have a [scope] section".into(),
        ));
    }

    Ok(())
}

/// Parse a `plugin.toml` file into a `PluginManifest`.
pub fn parse_manifest(path: &Path) -> Result<PluginManifest, ManifestError> {
    let contents = std::fs::read_to_string(path).map_err(|source| ManifestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let manifest: PluginManifest = toml::from_str(&contents)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}
