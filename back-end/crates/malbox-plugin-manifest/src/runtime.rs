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

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathsConfig {
    #[serde(default)]
    pub sample_dir: Option<PathBuf>,
    #[serde(default)]
    pub artifact_dir: Option<PathBuf>,
    #[serde(default)]
    pub stash_dir: Option<PathBuf>,
    #[serde(default)]
    pub log_dir: Option<PathBuf>,
    #[serde(default)]
    pub external_log_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StashConfig {
    #[serde(default)]
    pub threshold_bytes: Option<usize>,
    #[serde(default)]
    pub ttl_secs: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoCollectSectionConfig {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub include: Option<Vec<String>>,
    #[serde(default)]
    pub exclude: Option<Vec<String>>,
    #[serde(default)]
    pub max_file_size: Option<u64>,
    #[serde(default)]
    pub rules: Vec<AutoCollectRuleConfig>,
}

/// Per-file auto-collect rule: matching files are pushed as artifacts with
/// the given relation instead of plain outputs.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoCollectRuleConfig {
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    pub relation: String,
    #[serde(default)]
    pub analyze: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutoCollectConfig {
    #[serde(default)]
    pub artifacts: AutoCollectSectionConfig,
    #[serde(default)]
    pub external_logs: AutoCollectSectionConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    pub state: PluginStateConfig,
    pub execution: ExecutionContextConfig,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub stash: StashConfig,
    #[serde(default)]
    pub log_filter: Option<String>,
    #[serde(default)]
    pub auto_collect: AutoCollectConfig,
    #[serde(default)]
    pub analysis_timeout: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ResolvedAutoCollectSection {
    pub enabled: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub max_file_size: u64,
    pub rules: Vec<ResolvedAutoCollectRule>,
    /// Rule problems found during resolution, reported by `validate`.
    rule_errors: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedAutoCollectRule {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub relation: ArtifactRelation,
    pub analyze: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedRuntimeConfig {
    pub state: PluginStateConfig,
    pub execution: ExecutionContextConfig,
    pub port: u16,
    pub sample_dir: PathBuf,
    pub artifact_dir: PathBuf,
    pub stash_dir: PathBuf,
    pub log_dir: PathBuf,
    pub external_log_dir: PathBuf,
    pub stash_threshold_bytes: usize,
    pub stash_ttl_secs: u64,
    pub log_filter: String,
    pub analysis_timeout: u64,
    pub auto_collect_artifacts: ResolvedAutoCollectSection,
    pub auto_collect_external_logs: ResolvedAutoCollectSection,
}

impl ResolvedRuntimeConfig {
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
            auto_collect_artifacts: resolve_auto_collect_section(&raw.auto_collect.artifacts, true),
            auto_collect_external_logs: resolve_auto_collect_section(
                &raw.auto_collect.external_logs,
                true,
            ),
        }
    }

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

fn resolve_auto_collect_section(
    raw: &AutoCollectSectionConfig,
    default_enabled: bool,
) -> ResolvedAutoCollectSection {
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
        enabled: raw.enabled.unwrap_or(default_enabled),
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
