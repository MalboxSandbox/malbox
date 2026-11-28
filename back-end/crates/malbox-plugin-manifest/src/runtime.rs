//! `[runtime]` manifest section and its resolution into concrete defaults.

use crate::error::ManifestError;
use crate::manifest::{ExecutionContextConfig, PluginStateConfig, PluginTypeConfig};

use malbox_plugin_transport::artifact::ArtifactRelation;
use serde::Deserialize;
use std::path::{Path, PathBuf};

fn is_windows_absolute(path: &Path) -> bool {
    let s = path.to_string_lossy();
    let bytes = s.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn is_absolute_for_plugin(path: &Path, plugin_type: PluginTypeConfig) -> bool {
    if path.is_absolute() {
        return true;
    }
    match plugin_type {
        PluginTypeConfig::Guest => is_windows_absolute(path),
        PluginTypeConfig::Host => false,
    }
}

/// Optional overrides for plugin working directories; unset entries use platform defaults.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathsConfig {
    /// Directory where input samples are placed.
    #[serde(default)]
    pub sample_dir: Option<PathBuf>,
    /// Directory where the plugin writes produced artifacts.
    #[serde(default)]
    pub artifact_dir: Option<PathBuf>,
    /// Directory for large payloads stashed out of band.
    #[serde(default)]
    pub stash_dir: Option<PathBuf>,
    /// Directory for the plugin's own logs.
    #[serde(default)]
    pub log_dir: Option<PathBuf>,
    /// Directory for logs produced by external tools the plugin runs.
    #[serde(default)]
    pub external_log_dir: Option<PathBuf>,
}

/// Settings for stashing oversized results out of band.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StashConfig {
    /// Payload size above which results are stashed instead of sent inline.
    #[serde(default)]
    pub threshold_bytes: Option<usize>,
    /// Seconds a stashed payload is kept before cleanup.
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

/// Raw settings for automatically collecting files from one output directory.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoCollectSectionConfig {
    /// Whether collection runs; defaults to enabled.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Glob patterns of files to collect; defaults to `**/*`.
    #[serde(default)]
    pub include: Option<Vec<String>>,
    /// Glob patterns of files to skip.
    #[serde(default)]
    pub exclude: Option<Vec<String>>,
    /// Largest file size in bytes that is collected.
    #[serde(default)]
    pub max_file_size: Option<u64>,
    /// Rules that turn matching files into typed artifacts.
    #[serde(default)]
    pub rules: Vec<AutoCollectRuleConfig>,
}

/// Per-file auto-collect rule: matching files are pushed as artifacts with
/// the given relation instead of plain outputs.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoCollectRuleConfig {
    /// Glob patterns a file must match.
    pub include: Vec<String>,
    /// Glob patterns that exclude otherwise matching files.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Artifact relation label; `root` is rejected.
    pub relation: String,
    /// Whether the artifact is analyzed further; defaults per relation.
    #[serde(default)]
    pub analyze: Option<bool>,
}

/// Auto-collection settings for the artifact and external-log directories.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoCollectConfig {
    /// Collection from the artifact directory.
    #[serde(default)]
    pub artifacts: AutoCollectSectionConfig,
    /// Collection from the external-log directory; rules are not allowed.
    #[serde(default)]
    pub external_logs: AutoCollectSectionConfig,
}

/// Raw `[runtime]` section as written in `plugin.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    /// Process lifetime policy.
    pub state: PluginStateConfig,
    /// Task concurrency policy.
    pub execution: ExecutionContextConfig,
    /// gRPC listen port; defaults to 50051.
    #[serde(default)]
    pub port: Option<u16>,
    /// Working directory overrides.
    #[serde(default)]
    pub paths: PathsConfig,
    /// Out-of-band result stash settings.
    #[serde(default)]
    pub stash: StashConfig,
    /// `tracing` `EnvFilter` directive; defaults to `info`.
    #[serde(default)]
    pub log_filter: Option<String>,
    /// Automatic file collection settings.
    #[serde(default)]
    pub auto_collect: AutoCollectConfig,
    /// Default per-task timeout in seconds; defaults to 300.
    #[serde(default)]
    pub analysis_timeout: Option<u64>,
}

/// Auto-collect section with defaults applied and relations parsed.
#[derive(Debug, Clone)]
pub struct ResolvedAutoCollectSection {
    /// Whether collection runs.
    pub enabled: bool,
    /// Glob patterns of files to collect.
    pub include: Vec<String>,
    /// Glob patterns of files to skip.
    pub exclude: Vec<String>,
    /// Largest file size in bytes that is collected.
    pub max_file_size: u64,
    /// Rules with successfully parsed relations.
    pub rules: Vec<ResolvedAutoCollectRule>,
    /// Rule problems found during resolution, reported by `validate`.
    rule_errors: Vec<String>,
}

/// Auto-collect rule with its relation parsed and analyze default applied.
#[derive(Debug, Clone)]
pub struct ResolvedAutoCollectRule {
    /// Glob patterns a file must match.
    pub include: Vec<String>,
    /// Glob patterns that exclude otherwise matching files.
    pub exclude: Vec<String>,
    /// Relation recorded for collected artifacts.
    pub relation: ArtifactRelation,
    /// Whether collected artifacts are analyzed further.
    pub analyze: bool,
}

/// Runtime configuration with every default filled in.
#[derive(Debug, Clone)]
pub struct ResolvedRuntimeConfig {
    /// Process lifetime policy.
    pub state: PluginStateConfig,
    /// Task concurrency policy.
    pub execution: ExecutionContextConfig,
    /// gRPC listen port.
    pub port: u16,
    /// Directory where input samples are placed.
    pub sample_dir: PathBuf,
    /// Directory where produced artifacts are written.
    pub artifact_dir: PathBuf,
    /// Directory for stashed oversized payloads.
    pub stash_dir: PathBuf,
    /// Directory for the plugin's own logs.
    pub log_dir: PathBuf,
    /// Directory for logs of external tools.
    pub external_log_dir: PathBuf,
    /// Payload size above which results are stashed.
    pub stash_threshold_bytes: usize,
    /// Seconds a stashed payload is retained.
    pub stash_ttl_secs: u64,
    /// `tracing` `EnvFilter` directive.
    pub log_filter: String,
    /// Default per-task timeout in seconds.
    pub analysis_timeout: u64,
    /// Auto-collection from the artifact directory.
    pub auto_collect_artifacts: ResolvedAutoCollectSection,
    /// Auto-collection from the external-log directory.
    pub auto_collect_external_logs: ResolvedAutoCollectSection,
}

impl ResolvedRuntimeConfig {
    /// Apply defaults to a raw `[runtime]` section; problems surface in [`Self::validate`].
    #[must_use]
    pub fn from_raw(raw: &RuntimeConfig) -> Self {
        Self {
            state: raw.state,
            execution: raw.execution,
            port: raw.port.unwrap_or(50051),
            sample_dir: raw
                .paths
                .sample_dir
                .clone()
                .unwrap_or_else(default_sample_dir),
            artifact_dir: raw
                .paths
                .artifact_dir
                .clone()
                .unwrap_or_else(default_artifact_dir),
            stash_dir: raw
                .paths
                .stash_dir
                .clone()
                .unwrap_or_else(default_stash_dir),
            log_dir: raw.paths.log_dir.clone().unwrap_or_else(default_log_dir),
            external_log_dir: raw
                .paths
                .external_log_dir
                .clone()
                .unwrap_or_else(default_external_log_dir),
            stash_threshold_bytes: raw.stash.threshold_bytes.unwrap_or(1_048_576),
            stash_ttl_secs: raw.stash.ttl_secs.unwrap_or(120),
            log_filter: raw.log_filter.clone().unwrap_or_else(|| "info".into()),
            analysis_timeout: raw.analysis_timeout.unwrap_or(300),
            auto_collect_artifacts: resolve_auto_collect_section(&raw.auto_collect.artifacts),
            auto_collect_external_logs: resolve_auto_collect_section(
                &raw.auto_collect.external_logs,
            ),
        }
    }

    /// Check resolved values against the constraints for `plugin_type`.
    ///
    /// # Errors
    ///
    /// Returns [`ManifestError::Invalid`] if the port is below 1024, a path is not
    /// absolute for the plugin's platform, stash or timeout limits are too small,
    /// auto-collect rules are invalid or used on external logs, or the log filter
    /// does not parse.
    pub fn validate(&self, plugin_type: PluginTypeConfig) -> Result<(), ManifestError> {
        if self.port < 1024 {
            return Err(ManifestError::Invalid(format!(
                "runtime.port must be >= 1024, got {}",
                self.port
            )));
        }
        if !is_absolute_for_plugin(&self.sample_dir, plugin_type) {
            return Err(ManifestError::Invalid(format!(
                "runtime.paths.sample_dir must be absolute: {}",
                self.sample_dir.display()
            )));
        }
        if !is_absolute_for_plugin(&self.artifact_dir, plugin_type) {
            return Err(ManifestError::Invalid(format!(
                "runtime.paths.artifact_dir must be absolute: {}",
                self.artifact_dir.display()
            )));
        }
        if !is_absolute_for_plugin(&self.stash_dir, plugin_type) {
            return Err(ManifestError::Invalid(format!(
                "runtime.paths.stash_dir must be absolute: {}",
                self.stash_dir.display()
            )));
        }
        if !is_absolute_for_plugin(&self.log_dir, plugin_type) {
            return Err(ManifestError::Invalid(format!(
                "runtime.paths.log_dir must be absolute: {}",
                self.log_dir.display()
            )));
        }
        if !is_absolute_for_plugin(&self.external_log_dir, plugin_type) {
            return Err(ManifestError::Invalid(format!(
                "runtime.paths.external_log_dir must be absolute: {}",
                self.external_log_dir.display()
            )));
        }
        if self.stash_threshold_bytes < 4096 {
            return Err(ManifestError::Invalid(
                "runtime.stash.threshold_bytes must be >= 4096".into(),
            ));
        }
        if self.stash_ttl_secs < 1 {
            return Err(ManifestError::Invalid(
                "runtime.stash.ttl_secs must be >= 1".into(),
            ));
        }
        if self.analysis_timeout < 1 {
            return Err(ManifestError::Invalid(
                "runtime.analysis_timeout must be >= 1".into(),
            ));
        }
        let ext = &self.auto_collect_external_logs;
        if !ext.rules.is_empty() || !ext.rule_errors.is_empty() {
            return Err(ManifestError::Invalid(
                "rules are only supported for auto_collect.artifacts".into(),
            ));
        }
        let art = &self.auto_collect_artifacts;
        if let Some(e) = art.rule_errors.first() {
            return Err(ManifestError::Invalid(e.clone()));
        }
        for rule in &art.rules {
            for g in rule.include.iter().chain(&rule.exclude) {
                globset::Glob::new(g).map_err(|e| {
                    ManifestError::Invalid(format!(
                        "auto_collect.artifacts.rules glob '{g}' is invalid: {e}"
                    ))
                })?;
            }
        }
        tracing_subscriber::EnvFilter::try_new(&self.log_filter).map_err(|e| {
            ManifestError::Invalid(format!(
                "runtime.log_filter is not a valid EnvFilter ({e}): {}",
                self.log_filter
            ))
        })?;
        Ok(())
    }
}

const DEFAULT_AUTO_COLLECT_MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

fn resolve_auto_collect_section(raw: &AutoCollectSectionConfig) -> ResolvedAutoCollectSection {
    let mut rules = Vec::new();
    let mut rule_errors = Vec::new();
    for rule in &raw.rules {
        match relation_from_label(&rule.relation) {
            Some(relation) => rules.push(ResolvedAutoCollectRule {
                include: rule.include.clone(),
                exclude: rule.exclude.clone(),
                relation,
                analyze: rule.analyze.unwrap_or(relation.default_analyze()),
            }),
            None => rule_errors.push(format!("unknown artifact relation {}", rule.relation)),
        }
    }
    ResolvedAutoCollectSection {
        rules,
        rule_errors,
        enabled: raw.enabled.unwrap_or(true),
        include: raw.include.clone().unwrap_or_else(|| vec!["**/*".into()]),
        exclude: raw.exclude.clone().unwrap_or_default(),
        max_file_size: raw
            .max_file_size
            .unwrap_or(DEFAULT_AUTO_COLLECT_MAX_FILE_SIZE),
    }
}

/// Resolve a DB relation label; `root` is reserved and never matches.
fn relation_from_label(label: &str) -> Option<ArtifactRelation> {
    label
        .parse::<ArtifactRelation>()
        .ok()
        .filter(|r| *r != ArtifactRelation::Root)
}

#[cfg(unix)]
fn default_sample_dir() -> PathBuf {
    PathBuf::from("/tmp/malbox/samples")
}

#[cfg(unix)]
fn default_artifact_dir() -> PathBuf {
    PathBuf::from("/tmp/malbox/artifacts")
}

#[cfg(unix)]
fn default_stash_dir() -> PathBuf {
    PathBuf::from("/tmp/malbox/stash")
}

#[cfg(unix)]
fn default_log_dir() -> PathBuf {
    PathBuf::from("/tmp/malbox/logs")
}

#[cfg(unix)]
fn default_external_log_dir() -> PathBuf {
    PathBuf::from("/tmp/malbox/ext-logs")
}

#[cfg(windows)]
fn default_sample_dir() -> PathBuf {
    PathBuf::from(r"C:\malbox\samples")
}

#[cfg(windows)]
fn default_artifact_dir() -> PathBuf {
    PathBuf::from(r"C:\malbox\artifacts")
}

#[cfg(windows)]
fn default_stash_dir() -> PathBuf {
    PathBuf::from(r"C:\malbox\stash")
}

#[cfg(windows)]
fn default_log_dir() -> PathBuf {
    PathBuf::from(r"C:\malbox\logs")
}

#[cfg(windows)]
fn default_external_log_dir() -> PathBuf {
    PathBuf::from(r"C:\malbox\ext-logs")
}
