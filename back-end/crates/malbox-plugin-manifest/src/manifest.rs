//! `plugin.toml` schema types and structural validation.

use crate::error::ManifestError;
use crate::runtime::RuntimeConfig;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

/// Top-level manifest structure parsed from `plugin.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    /// Core identity and type metadata from the `[plugin]` table.
    pub plugin: PluginInfo,
    #[serde(default)]
    /// Scope restrictions; required when the plugin state is `scoped`.
    pub scope: Option<ScopeConfig>,
    #[serde(default)]
    /// Result keys the plugin may emit, keyed by result name.
    pub results: HashMap<String, ResultConfig>,
    /// Runtime lifecycle, execution and path settings.
    pub runtime: RuntimeConfig,
    #[serde(default)]
    /// Host plugin routing rules; must stay default for guest plugins.
    pub routing: RoutingConfig,
    /// User-tunable parameters with typed defaults, keyed by parameter name.
    #[serde(default)]
    pub parameters: BTreeMap<String, ParameterConfig>,
}

/// Core plugin metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginInfo {
    /// Unique plugin name; alphanumeric, `-` and `_` only.
    pub name: String,
    /// Semver version string of the plugin.
    pub version: String,
    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Plugin authors as free-form strings.
    #[serde(default)]
    pub authors: Vec<String>,
    /// Whether the plugin runs on the host or inside a guest VM.
    #[serde(rename = "type")]
    pub plugin_type: PluginTypeConfig,
    /// Optional explicit binary name (defaults to directory name).
    #[serde(default)]
    pub binary: Option<String>,
}

/// Where a plugin executes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox-scheduler routing"
)]
pub enum PluginTypeConfig {
    /// Runs as a process on the analysis host.
    Host,
    /// Runs inside a guest analysis VM.
    Guest,
}

/// Plugin process lifetime policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "lifecycle state matched exhaustively by malbox-plugin-internal and malbox-plugin-macros"
)]
pub enum PluginStateConfig {
    /// One long-lived process serves all tasks.
    Persistent,
    /// A fresh process is spawned per task.
    Ephemeral,
    /// Process lifetime is bound to the `[scope]` section.
    Scoped,
}

/// Concurrency policy for tasks dispatched to a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox-plugin-macros codegen"
)]
pub enum ExecutionContextConfig {
    /// No other plugin runs while this one runs.
    Exclusive,
    /// Tasks for this plugin run one at a time.
    Sequential,
    /// Tasks may run concurrently alongside other parallel plugins.
    Parallel,
    /// No concurrency restrictions at all.
    Unrestricted,
}

/// Restricts which plugins and task types a scoped plugin is bound to.
#[derive(Debug, Clone, Deserialize)]
pub struct ScopeConfig {
    /// Plugin names sharing this plugin's scope.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// Task types this plugin is scoped to.
    #[serde(default)]
    pub task_types: Vec<String>,
}

/// Presentation metadata for one result key a plugin emits.
#[derive(Debug, Clone, Deserialize)]
pub struct ResultConfig {
    /// Human-readable explanation of the result.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether the result is shown to users; `None` uses the front-end default.
    #[serde(default)]
    pub user_visible: Option<bool>,
    /// Label shown in the UI instead of the raw key.
    #[serde(default)]
    pub display_name: Option<String>,
    /// Renderer hint for the front-end.
    #[serde(default)]
    pub render: Option<String>,
}

/// Analysis stage, in release order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Stage {
    /// Early triage that can drop artifacts.
    Filter,
    /// Unpacking and extraction of embedded content.
    Extract,
    /// Static analysis of artifact contents.
    Static,
    /// Detonation in a VM; reserved, not selectable by plugins.
    Dynamic,
    /// Post-processing of earlier results.
    Post,
    /// Final review and scoring.
    Review,
}

impl Stage {
    /// Every stage in execution order.
    pub const ALL: [Stage; 6] = [
        Stage::Filter,
        Stage::Extract,
        Stage::Static,
        Stage::Dynamic,
        Stage::Post,
        Stage::Review,
    ];

    /// Lowercase name as used in manifests and as the default category.
    #[must_use]
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
    /// Stage the plugin runs in.
    #[serde(default = "default_stage")]
    pub stage: Stage,
    /// Routing category; `None` means the stage name.
    #[serde(default)]
    pub category: Option<String>,
    /// Regex over artifact types the plugin runs on, anchored on both ends.
    #[serde(default = "default_accepts")]
    pub accepts: String,
    /// Regex over artifact types to skip; empty rejects nothing.
    #[serde(default)]
    pub rejects: String,
    /// Plugin names whose outputs this plugin skips.
    #[serde(default)]
    pub sources_exclude: Vec<String>,
    /// Per-task timeout in seconds; `None` means `runtime.analysis_timeout` or 300.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Whether results may be reused for identical inputs.
    #[serde(default = "default_true")]
    pub cacheable: bool,
    /// Categories forbidden for descendants; `"dynamic"` means detonations.
    #[serde(default)]
    pub recursion_prevention: Vec<String>,
    /// Whether the plugin consumes indicators produced by other plugins.
    #[serde(default)]
    pub uses_indicators: bool,
    /// Result keys whose changes retrigger this plugin.
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
    /// Effective category: the explicit one, else the stage name.
    #[must_use]
    pub fn category(&self) -> &str {
        self.category.as_deref().unwrap_or(self.stage.as_str())
    }
}

/// Declaration of one user-tunable plugin parameter.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParameterConfig {
    /// Value type the parameter accepts.
    #[serde(rename = "type")]
    pub kind: ParameterKind,
    /// Default value; must match `kind`.
    pub default: toml::Value,
    /// Optional human-readable explanation.
    #[serde(default)]
    pub description: Option<String>,
}

/// Value type of a plugin parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox-scheduler parameter validation"
)]
pub enum ParameterKind {
    /// Boolean value.
    Bool,
    /// Signed integer value.
    Int,
    /// String value.
    String,
    /// List of strings.
    List,
}

impl ParameterKind {
    fn matches(self, v: &toml::Value) -> bool {
        match (self, v) {
            (ParameterKind::Bool, toml::Value::Boolean(_))
            | (ParameterKind::Int, toml::Value::Integer(_))
            | (ParameterKind::String, toml::Value::String(_)) => true,
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
///
/// # Errors
///
/// Returns [`ManifestError::Invalid`] for an empty or malformed name, a non-semver
/// version, a scoped plugin without `[scope]`, invalid routing (dynamic stage,
/// routing on a guest, bad regex), or a badly named or mistyped parameter.
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
///
/// # Errors
///
/// Returns [`ManifestError::Io`] if the file cannot be read, [`ManifestError::Parse`]
/// on invalid TOML, and [`ManifestError::Invalid`] if validation fails.
pub fn parse_manifest(path: &Path) -> Result<PluginManifest, ManifestError> {
    let contents = std::fs::read_to_string(path).map_err(|source| ManifestError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let manifest: PluginManifest = toml::from_str(&contents)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}
