use malbox_plugin_manifest::{
    ManifestError, PluginManifest, PluginTypeConfig, ResolvedRuntimeConfig, RuntimeConfig,
    parse_manifest, validate_manifest,
};
use std::io::Write;
use std::path::PathBuf;

fn valid_manifest_toml() -> &'static str {
    r#"
[plugin]
name = "pe-parser"
version = "1.0.0"
type = "host"
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
fn parses_minimal_manifest_without_runtime_section() {
    let f = write_toml(
        r#"
[plugin]
name = "example"
version = "0.1.0"
type = "guest"
state = "ephemeral"
execution = "exclusive"
"#,
    );
    let m = parse_manifest(f.path()).expect("parse");
    assert_eq!(m.plugin.name, "example");
    assert_eq!(m.plugin.plugin_type, PluginTypeConfig::Guest);
    assert!(m.runtime.is_none());
}

#[test]
fn resolve_fills_defaults_when_all_none() {
    let raw = RuntimeConfig::default();
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert_eq!(r.port, 50051);
    #[cfg(unix)]
    assert_eq!(r.work_dir, PathBuf::from("/tmp/malbox"));
    #[cfg(windows)]
    assert_eq!(r.work_dir, PathBuf::from(r"C:\malbox\work"));
    assert_eq!(r.log_overflow_dir, r.work_dir.join("_logs"));
    assert_eq!(r.stash_threshold_bytes, 1_048_576);
    assert_eq!(r.stash_ttl_secs, 120);
    assert_eq!(r.log_filter, "info");
}

#[test]
fn resolve_respects_explicit_fields() {
    let raw = RuntimeConfig {
        port: Some(50100),
        work_dir: Some(PathBuf::from("/opt/malbox")),
        log_overflow_dir: Some(PathBuf::from("/var/log/malbox")),
        stash_threshold_bytes: Some(2_000_000),
        stash_ttl_secs: Some(300),
        log_filter: Some("debug".into()),
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert_eq!(r.port, 50100);
    assert_eq!(r.work_dir, PathBuf::from("/opt/malbox"));
    assert_eq!(r.log_overflow_dir, PathBuf::from("/var/log/malbox"));
    assert_eq!(r.stash_threshold_bytes, 2_000_000);
    assert_eq!(r.stash_ttl_secs, 300);
    assert_eq!(r.log_filter, "debug");
}

#[test]
fn resolve_derives_log_overflow_from_work_dir_when_missing() {
    let raw = RuntimeConfig {
        work_dir: Some(PathBuf::from("/opt/malbox")),
        ..RuntimeConfig::default()
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert_eq!(r.log_overflow_dir, PathBuf::from("/opt/malbox/_logs"));
}

#[test]
fn validate_rejects_privileged_port() {
    let raw = RuntimeConfig {
        port: Some(80),
        ..RuntimeConfig::default()
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    let err = r.validate().unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("port")),
        "got {err:?}"
    );
}

#[test]
fn validate_rejects_relative_work_dir() {
    let raw = RuntimeConfig {
        work_dir: Some(PathBuf::from("relative/path")),
        ..RuntimeConfig::default()
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    let err = r.validate().unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("absolute")),
        "got {err:?}"
    );
}

#[test]
fn validate_rejects_bad_log_filter() {
    let raw = RuntimeConfig {
        log_filter: Some("!!!not a filter!!!".into()),
        ..RuntimeConfig::default()
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert!(r.validate().is_err());
}

#[test]
fn validate_accepts_good_config() {
    let raw = RuntimeConfig {
        port: Some(50100),
        work_dir: Some(PathBuf::from("/opt/malbox")),
        log_filter: Some("info,hyper=warn".into()),
        ..RuntimeConfig::default()
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert!(r.validate().is_ok());
}
