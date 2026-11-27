use crate::error::ManifestError;
use crate::runtime::RuntimeConfig;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Top-level manifest structure parsed from `plugin.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub plugin: PluginInfo,
    #[serde(default)]
    pub scope: Option<ScopeConfig>,
    #[serde(default)]
    pub results: HashMap<String, ResultConfig>,
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub routing: RoutingConfig,
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterConfig>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginTypeConfig {
    Host,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginStateConfig {
    Persistent,
    Ephemeral,
    Scoped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Analysis stage, in release order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Filter,
    Extract,
    Static,
    Dynamic,
    Post,
    Review,
}

impl Stage {
    pub const ALL: [Stage; 6] = [
        Stage::Filter,
        Stage::Extract,
        Stage::Static,
        Stage::Dynamic,
        Stage::Post,
        Stage::Review,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Filter => "filter",
            Stage::Extract => "extract",
            Stage::Static => "static",
            Stage::Dynamic => "dynamic",
            Stage::Post => "post",
            Stage::Review => "review",
        }
    }
}

fn default_stage() -> Stage {
    Stage::Static
}

fn default_accepts() -> String {
    ".*".into()
}

fn default_true() -> bool {
    true
}

/// Host plugin routing: which artifacts it runs on and when.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingConfig {
    #[serde(default = "default_stage")]
    pub stage: Stage,
    /// `None` means the stage name.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default = "default_accepts")]
    pub accepts: String,
    /// Empty rejects nothing.
    #[serde(default)]
    pub rejects: String,
    /// Plugin names whose outputs this plugin skips.
    #[serde(default)]
    pub sources_exclude: Vec<String>,
    /// `None` means `runtime.analysis_timeout` or 300.
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default = "default_true")]
    pub cacheable: bool,
    /// Categories forbidden for descendants; `"dynamic"` means detonations.
    #[serde(default)]
    pub recursion_prevention: Vec<String>,
    #[serde(default)]
    pub uses_indicators: bool,
    #[serde(default)]
    pub monitored_keys: Vec<String>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            stage: default_stage(),
            category: None,
            accepts: default_accepts(),
            rejects: String::new(),
            sources_exclude: Vec::new(),
            timeout: None,
            cacheable: true,
            recursion_prevention: Vec::new(),
            uses_indicators: false,
            monitored_keys: Vec::new(),
        }
    }
}

impl RoutingConfig {
    pub fn category(&self) -> &str {
        self.category.as_deref().unwrap_or(self.stage.as_str())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterConfig {
    #[serde(rename = "type")]
    pub kind: ParameterKind,
    pub default: toml::Value,
    #[serde(default)]
    pub description: Option<String>,
}

/// `List` is a list of strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParameterKind {
    Bool,
    Int,
    String,
    List,
}

impl ParameterKind {
    fn matches(self, v: &toml::Value) -> bool {
        match (self, v) {
            (ParameterKind::Bool, toml::Value::Boolean(_)) => true,
            (ParameterKind::Int, toml::Value::Integer(_)) => true,
            (ParameterKind::String, toml::Value::String(_)) => true,
            (ParameterKind::List, toml::Value::Array(items)) => {
                items.iter().all(|i| matches!(i, toml::Value::String(_)))
            }
            _ => false,
        }
    }
}

fn check_routing_regex(field: &str, expr: &str) -> Result<(), ManifestError> {
    regex::Regex::new(&format!("^(?:{expr})$"))
        .map(|_| ())
        .map_err(|e| ManifestError::Invalid(format!("routing.{field} is not a valid regex: {e}")))
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

    let routing = &manifest.routing;
    if routing.stage == Stage::Dynamic {
        return Err(ManifestError::Invalid(
            "routing.stage = dynamic is reserved for detonation".into(),
        ));
    }
    if manifest.plugin.plugin_type == PluginTypeConfig::Guest
        && *routing != RoutingConfig::default()
    {
        return Err(ManifestError::Invalid(
            "[routing] is only valid for host plugins".into(),
        ));
    }
    check_routing_regex("accepts", &routing.accepts)?;
    if !routing.rejects.is_empty() {
        check_routing_regex("rejects", &routing.rejects)?;
    }

    for (name, param) in &manifest.parameters {
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(ManifestError::Invalid(format!(
                "parameter {name}: name must match [A-Za-z0-9_]+"
            )));
        }
        if !param.kind.matches(&param.default) {
            return Err(ManifestError::Invalid(format!(
                "parameter {name}: default does not match type"
            )));
        }
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
