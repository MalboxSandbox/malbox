use malbox_plugin_manifest::{
    ManifestError, PathsConfig, PluginManifest, PluginStateConfig, PluginTypeConfig,
    ResolvedRuntimeConfig, RuntimeConfig, StashConfig, parse_manifest, validate_manifest,
};
use std::io::Write;
use std::path::PathBuf;

fn valid_manifest_toml() -> &'static str {
    r#"
[plugin]
name = "pe-parser"
version = "1.0.0"
type = "host"

[runtime]
state = "persistent"
execution = "parallel"
"#
}

#[test]
fn validate_accepts_valid_manifest() {
    let m: PluginManifest = toml::from_str(valid_manifest_toml()).unwrap();
    assert!(validate_manifest(&m).is_ok());
}

#[test]
fn validate_rejects_empty_name() {
    let toml_str = valid_manifest_toml().replace("pe-parser", "");
    let m: PluginManifest = toml::from_str(&toml_str).unwrap();
    let err = validate_manifest(&m).unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("empty")),
        "got {err:?}"
    );
}

#[test]
fn validate_rejects_bad_version() {
    let toml_str = valid_manifest_toml().replace("1.0.0", "not-a-version");
    let m: PluginManifest = toml::from_str(&toml_str).unwrap();
    let err = validate_manifest(&m).unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("version")),
        "got {err:?}"
    );
}

#[test]
fn validate_rejects_scoped_without_scope_section() {
    let toml_str = valid_manifest_toml().replace("persistent", "scoped");
    let m: PluginManifest = toml::from_str(&toml_str).unwrap();
    let err = validate_manifest(&m).unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("scope")),
        "got {err:?}"
    );
}

#[test]
fn parse_manifest_with_results() {
    let toml_str = r#"
[plugin]
name = "pe-parser"
version = "1.0.0"
type = "host"

[runtime]
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

[runtime]
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
fn parse_manifest_file_not_found() {
    let result = parse_manifest(std::path::Path::new("/nonexistent/plugin.toml"));
    assert!(result.is_err());
}

fn write_toml(contents: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    f
}

#[test]
fn parses_manifest_with_runtime_section() {
    let f = write_toml(
        r#"
[plugin]
name = "example"
version = "0.1.0"
type = "guest"

[runtime]
state = "ephemeral"
execution = "exclusive"
"#,
    );
    let m = parse_manifest(f.path()).expect("parse");
    assert_eq!(m.plugin.name, "example");
    assert_eq!(m.plugin.plugin_type, PluginTypeConfig::Guest);
    assert_eq!(m.runtime.state, PluginStateConfig::Ephemeral);
}

fn make_runtime(state: &str, execution: &str) -> RuntimeConfig {
    toml::from_str(&format!(
        r#"
state = "{state}"
execution = "{execution}"
"#
    ))
    .unwrap()
}

#[test]
fn resolve_fills_defaults_when_all_none() {
    let raw = make_runtime("ephemeral", "exclusive");
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert_eq!(r.port, 50051);
    #[cfg(unix)]
    assert_eq!(r.sample_dir, PathBuf::from("/tmp/malbox/samples"));
    #[cfg(windows)]
    assert_eq!(r.sample_dir, PathBuf::from(r"C:\malbox\samples"));
    assert_eq!(r.stash_threshold_bytes, 1_048_576);
    assert_eq!(r.stash_ttl_secs, 120);
    assert_eq!(r.log_filter, "info");
}

#[test]
fn resolve_respects_explicit_fields() {
    let raw = RuntimeConfig {
        state: PluginStateConfig::Ephemeral,
        execution: malbox_plugin_manifest::ExecutionContextConfig::Exclusive,
        port: Some(50100),
        paths: PathsConfig {
            sample_dir: Some(PathBuf::from("/opt/malbox/samples")),
            artifact_dir: Some(PathBuf::from("/opt/malbox/artifacts")),
            stash_dir: Some(PathBuf::from("/opt/malbox/stash")),
            log_dir: Some(PathBuf::from("/opt/malbox/logs")),
        },
        stash: StashConfig {
            threshold_bytes: Some(2_000_000),
            ttl_secs: Some(300),
        },
        log_filter: Some("debug".into()),
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert_eq!(r.port, 50100);
    assert_eq!(r.sample_dir, PathBuf::from("/opt/malbox/samples"));
    assert_eq!(r.artifact_dir, PathBuf::from("/opt/malbox/artifacts"));
    assert_eq!(r.stash_dir, PathBuf::from("/opt/malbox/stash"));
    assert_eq!(r.log_dir, PathBuf::from("/opt/malbox/logs"));
    assert_eq!(r.stash_threshold_bytes, 2_000_000);
    assert_eq!(r.stash_ttl_secs, 300);
    assert_eq!(r.log_filter, "debug");
}

#[test]
fn validate_rejects_privileged_port() {
    let mut raw = make_runtime("ephemeral", "exclusive");
    raw.port = Some(80);
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    let err = r.validate().unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("port")),
        "got {err:?}"
    );
}

#[test]
fn validate_rejects_relative_sample_dir() {
    let mut raw = make_runtime("ephemeral", "exclusive");
    raw.paths.sample_dir = Some(PathBuf::from("relative/path"));
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    let err = r.validate().unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("absolute")),
        "got {err:?}"
    );
}

#[test]
fn validate_rejects_bad_log_filter() {
    let mut raw = make_runtime("ephemeral", "exclusive");
    raw.log_filter = Some("!!!not a filter!!!".into());
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert!(r.validate().is_err());
}

#[test]
fn validate_accepts_good_config() {
    let mut raw = make_runtime("ephemeral", "exclusive");
    raw.port = Some(50100);
    raw.log_filter = Some("info,hyper=warn".into());
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert!(r.validate().is_ok());
}
