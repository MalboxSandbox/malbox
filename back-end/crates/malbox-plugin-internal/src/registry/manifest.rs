use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use super::error::ManifestError;

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
    pub state: PluginStateConfig,
    pub execution: ExecutionContextConfig,
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

#[derive(Debug, Clone, Deserialize)]
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

/// Parse a `plugin.toml` file into a validated `PluginManifest`.
pub fn parse_manifest(path: &Path) -> Result<PluginManifest, ManifestError> {
    let content = std::fs::read_to_string(path).map_err(ManifestError::Io)?;
    let manifest: PluginManifest = toml::from_str(&content).map_err(ManifestError::Parse)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Validate structural correctness of a parsed manifest.
pub fn validate_manifest(manifest: &PluginManifest) -> Result<(), ManifestError> {
    if manifest.plugin.name.is_empty() {
        return Err(ManifestError::Validation(
            "plugin name must not be empty".into(),
        ));
    }

    if !manifest
        .plugin
        .name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ManifestError::Validation(format!(
            "plugin name '{}' contains invalid characters (allowed: alphanumeric, '-', '_')",
            manifest.plugin.name
        )));
    }

    semver::Version::parse(&manifest.plugin.version).map_err(|e| {
        ManifestError::Validation(format!(
            "invalid version '{}': {}",
            manifest.plugin.version, e
        ))
    })?;

    if manifest.plugin.state == PluginStateConfig::Scoped && manifest.scope.is_none() {
        return Err(ManifestError::Validation(
            "plugin with state 'scoped' must have a [scope] section".into(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_MANIFEST: &str = r#"
[plugin]
name = "pe-parser"
version = "1.0.0"
description = "Parses PE file headers"
authors = ["Malbox Team"]
type = "host"
state = "persistent"
execution = "parallel"
"#;

    const MINIMAL_MANIFEST: &str = r#"
[plugin]
name = "minimal"
version = "0.1.0"
type = "host"
state = "ephemeral"
execution = "unrestricted"
"#;

    #[test]
    fn parse_valid_manifest() {
        let manifest: PluginManifest = toml::from_str(VALID_MANIFEST).unwrap();
        assert_eq!(manifest.plugin.name, "pe-parser");
        assert_eq!(manifest.plugin.version, "1.0.0");
        assert_eq!(
            manifest.plugin.description.as_deref(),
            Some("Parses PE file headers")
        );
        assert_eq!(manifest.plugin.authors, vec!["Malbox Team"]);
    }

    #[test]
    fn parse_minimal_manifest() {
        let manifest: PluginManifest = toml::from_str(MINIMAL_MANIFEST).unwrap();
        assert_eq!(manifest.plugin.name, "minimal");
        assert!(manifest.plugin.description.is_none());
        assert!(manifest.plugin.authors.is_empty());
        assert!(manifest.scope.is_none());
        assert!(manifest.results.is_empty());
        assert!(manifest.events.is_none());
    }

    #[test]
    fn parse_plugin_type_host() {
        let manifest: PluginManifest = toml::from_str(VALID_MANIFEST).unwrap();
        assert_eq!(manifest.plugin.plugin_type, PluginTypeConfig::Host);
    }

    #[test]
    fn parse_plugin_type_guest() {
        let toml_str = VALID_MANIFEST.replace("host", "guest");
        let manifest: PluginManifest = toml::from_str(&toml_str).unwrap();
        assert_eq!(manifest.plugin.plugin_type, PluginTypeConfig::Guest);
    }

    #[test]
    fn parse_execution_contexts() {
        for (input, expected) in [
            ("exclusive", ExecutionContextConfig::Exclusive),
            ("sequential", ExecutionContextConfig::Sequential),
            ("parallel", ExecutionContextConfig::Parallel),
            ("unrestricted", ExecutionContextConfig::Unrestricted),
        ] {
            let toml_str = VALID_MANIFEST.replace("parallel", input);
            let manifest: PluginManifest = toml::from_str(&toml_str).unwrap();
            assert_eq!(manifest.plugin.execution, expected);
        }
    }

    #[test]
    fn parse_state_types() {
        for (input, expected) in [
            ("persistent", PluginStateConfig::Persistent),
            ("ephemeral", PluginStateConfig::Ephemeral),
            ("scoped", PluginStateConfig::Scoped),
        ] {
            let toml_str = VALID_MANIFEST.replace("persistent", input);
            let manifest: PluginManifest = toml::from_str(&toml_str).unwrap();
            assert_eq!(manifest.plugin.state, expected);
        }
    }

    #[test]
    fn validate_valid_manifest() {
        let manifest: PluginManifest = toml::from_str(VALID_MANIFEST).unwrap();
        assert!(validate_manifest(&manifest).is_ok());
    }

    #[test]
    fn validate_empty_name_fails() {
        let toml_str = VALID_MANIFEST.replace("pe-parser", "");
        let manifest: PluginManifest = toml::from_str(&toml_str).unwrap();
        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn validate_bad_version_fails() {
        let toml_str = VALID_MANIFEST.replace("1.0.0", "not-a-version");
        let manifest: PluginManifest = toml::from_str(&toml_str).unwrap();
        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn parse_manifest_with_results() {
        let toml_str = r#"
[plugin]
name = "pe-parser"
version = "1.0.0"
type = "host"
state = "persistent"
execution = "parallel"

[results.pe_info]
description = "Parsed PE header information"

[results.report]
description = "PE Analysis Report"
user_visible = true
display_name = "PE Analysis"
render = "json"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.results.len(), 2);
        assert!(manifest.results.contains_key("pe_info"));
        let report = &manifest.results["report"];
        assert_eq!(report.user_visible, Some(true));
        assert_eq!(report.display_name.as_deref(), Some("PE Analysis"));
    }

    #[test]
    fn parse_manifest_with_events() {
        let toml_str = r#"
[plugin]
name = "aggregator"
version = "1.0.0"
type = "host"
state = "persistent"
execution = "parallel"

[events]
subscribe = ["TaskCompleted", "ResultProduced"]
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let events = manifest.events.unwrap();
        assert_eq!(events.subscribe, vec!["TaskCompleted", "ResultProduced"]);
    }

    #[test]
    fn parse_manifest_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let manifest_path = dir.path().join("plugin.toml");
        std::fs::write(&manifest_path, VALID_MANIFEST).unwrap();

        let manifest = parse_manifest(&manifest_path).unwrap();
        assert_eq!(manifest.plugin.name, "pe-parser");
    }

    #[test]
    fn parse_manifest_file_not_found() {
        let result = parse_manifest(std::path::Path::new("/nonexistent/plugin.toml"));
        assert!(result.is_err());
    }
}
