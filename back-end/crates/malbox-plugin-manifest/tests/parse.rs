//! Tests for `plugin.toml` parsing, validation and runtime resolution.

use malbox_plugin_manifest::{
    AutoCollectConfig, ManifestError, ParameterKind, PathsConfig, PluginManifest,
    PluginStateConfig, PluginTypeConfig, ResolvedRuntimeConfig, RuntimeConfig, Stage, StashConfig,
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
fn parse_manifest_file_not_found() {
    let result = parse_manifest(std::path::Path::new("/nonexistent/plugin.toml"));
    assert!(result.is_err());
}

fn write_toml(contents: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().expect("create temp file");
    f.write_all(contents.as_bytes())
        .expect("write temp manifest");
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
    .expect("valid runtime toml")
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
            external_log_dir: Some(PathBuf::from("/opt/malbox/ext-logs")),
        },
        stash: StashConfig {
            threshold_bytes: Some(2_000_000),
            ttl_secs: Some(300),
        },
        log_filter: Some("debug".into()),
        auto_collect: AutoCollectConfig::default(),
        analysis_timeout: None,
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert_eq!(r.port, 50100);
    assert_eq!(r.sample_dir, PathBuf::from("/opt/malbox/samples"));
    assert_eq!(r.artifact_dir, PathBuf::from("/opt/malbox/artifacts"));
    assert_eq!(r.stash_dir, PathBuf::from("/opt/malbox/stash"));
    assert_eq!(r.log_dir, PathBuf::from("/opt/malbox/logs"));
    assert_eq!(r.external_log_dir, PathBuf::from("/opt/malbox/ext-logs"));
    assert_eq!(r.stash_threshold_bytes, 2_000_000);
    assert_eq!(r.stash_ttl_secs, 300);
    assert_eq!(r.log_filter, "debug");
}

#[test]
fn validate_rejects_privileged_port() {
    let mut raw = make_runtime("ephemeral", "exclusive");
    raw.port = Some(80);
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    let err = r.validate(PluginTypeConfig::Host).unwrap_err();
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
    let err = r.validate(PluginTypeConfig::Host).unwrap_err();
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
    assert!(r.validate(PluginTypeConfig::Host).is_err());
}

#[test]
fn validate_accepts_good_config() {
    let mut raw = make_runtime("ephemeral", "exclusive");
    raw.port = Some(50100);
    raw.log_filter = Some("info,hyper=warn".into());
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert!(r.validate(PluginTypeConfig::Host).is_ok());
}

#[test]
fn validate_accepts_windows_paths_for_guest_plugin() {
    let raw = RuntimeConfig {
        state: PluginStateConfig::Ephemeral,
        execution: malbox_plugin_manifest::ExecutionContextConfig::Exclusive,
        port: Some(50051),
        paths: PathsConfig {
            sample_dir: Some(PathBuf::from(r"C:\ProgramData")),
            artifact_dir: Some(PathBuf::from(r"C:\ProgramData\Agent\Artifacts")),
            stash_dir: Some(PathBuf::from(r"C:\ProgramData\Agent\Stash")),
            log_dir: Some(PathBuf::from(r"C:\ProgramData\Agent\Logs")),
            external_log_dir: Some(PathBuf::from(r"C:\ProgramData\Agent\ExtLogs")),
        },
        stash: StashConfig {
            threshold_bytes: None,
            ttl_secs: None,
        },
        log_filter: Some("info".into()),
        auto_collect: AutoCollectConfig::default(),
        analysis_timeout: None,
    };
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert!(r.validate(PluginTypeConfig::Guest).is_ok());
}

#[test]
fn validate_rejects_windows_paths_for_host_plugin() {
    let mut raw = make_runtime("ephemeral", "exclusive");
    raw.paths.sample_dir = Some(PathBuf::from(r"C:\ProgramData"));
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    let err = r.validate(PluginTypeConfig::Host).unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("absolute")),
        "got {err:?}"
    );
}

#[test]
fn validate_rejects_relative_paths_for_guest_plugin() {
    let mut raw = make_runtime("ephemeral", "exclusive");
    raw.paths.sample_dir = Some(PathBuf::from("relative/path"));
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    let err = r.validate(PluginTypeConfig::Guest).unwrap_err();
    assert!(
        matches!(err, ManifestError::Invalid(ref msg) if msg.contains("absolute")),
        "got {err:?}"
    );
}

#[test]
fn resolve_auto_collect_defaults() {
    let raw = make_runtime("ephemeral", "exclusive");
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert!(r.auto_collect_artifacts.enabled);
    assert_eq!(r.auto_collect_artifacts.include, vec!["**/*"]);
    assert!(r.auto_collect_artifacts.exclude.is_empty());
    assert_eq!(r.auto_collect_artifacts.max_file_size, 50 * 1024 * 1024);
    assert!(r.auto_collect_external_logs.enabled);
}

#[test]
fn parse_auto_collect_from_toml() {
    let toml_str = r#"
state = "ephemeral"
execution = "exclusive"

[auto_collect.artifacts]
enabled = false
include = ["*.bin", "*.exe"]
exclude = ["*.tmp"]
max_file_size = 10485760

[auto_collect.external_logs]
enabled = true
exclude = ["*.lock"]
"#;
    let raw: RuntimeConfig = toml::from_str(toml_str).unwrap();
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    assert!(!r.auto_collect_artifacts.enabled);
    assert_eq!(r.auto_collect_artifacts.include, vec!["*.bin", "*.exe"]);
    assert_eq!(r.auto_collect_artifacts.exclude, vec!["*.tmp"]);
    assert_eq!(r.auto_collect_artifacts.max_file_size, 10_485_760);
    assert!(r.auto_collect_external_logs.enabled);
    assert_eq!(r.auto_collect_external_logs.include, vec!["**/*"]);
    assert_eq!(r.auto_collect_external_logs.exclude, vec!["*.lock"]);
}

#[test]
fn resolve_external_log_dir_default() {
    let raw = make_runtime("ephemeral", "exclusive");
    let r = ResolvedRuntimeConfig::from_raw(&raw);
    #[cfg(unix)]
    assert_eq!(r.external_log_dir, PathBuf::from("/tmp/malbox/ext-logs"));
}

const HOST_HEAD: &str = r#"
[plugin]
name = "p"
version = "1.0.0"
type = "host"

[runtime]
state = "ephemeral"
execution = "parallel"
"#;

fn parse_str(extra: &str, head: &str) -> Result<PluginManifest, ManifestError> {
    let m: PluginManifest = toml::from_str(&format!("{head}{extra}"))?;
    validate_manifest(&m)?;
    Ok(m)
}

fn assert_invalid(r: Result<PluginManifest, ManifestError>, needle: &str) {
    match r {
        Err(ManifestError::Invalid(msg)) => assert!(msg.contains(needle), "{msg}"),
        other => panic!("expected Invalid({needle}), got {other:?}"),
    }
}

#[test]
fn parses_routing_and_parameters() {
    let m = parse_str(
        r#"
[routing]
stage = "extract"
category = "unpack"
accepts = "archive/.*"
rejects = "archive/iso"
sources_exclude = ["other"]
timeout = 60
cacheable = false
recursion_prevention = ["dynamic"]
uses_indicators = true
monitored_keys = ["passwords"]

[parameters.passwords]
type = "list"
default = ["a"]
description = "pw"

[parameters.deep]
type = "bool"
default = false
"#,
        HOST_HEAD,
    )
    .unwrap();
    let r = &m.routing;
    assert_eq!(r.stage, Stage::Extract);
    assert_eq!(r.category(), "unpack");
    assert_eq!(r.accepts, "archive/.*");
    assert_eq!(r.rejects, "archive/iso");
    assert_eq!(r.sources_exclude, vec!["other"]);
    assert_eq!(r.timeout, Some(60));
    assert!(!r.cacheable);
    assert_eq!(r.recursion_prevention, vec!["dynamic"]);
    assert!(r.uses_indicators);
    assert_eq!(r.monitored_keys, vec!["passwords"]);
    assert_eq!(m.parameters.len(), 2);
    assert_eq!(m.parameters["passwords"].kind, ParameterKind::List);
    assert_eq!(m.parameters["passwords"].description.as_deref(), Some("pw"));
    assert_eq!(m.parameters["deep"].kind, ParameterKind::Bool);
    assert_eq!(m.parameters["deep"].default, toml::Value::Boolean(false));
}

#[test]
fn rejects_dynamic_stage() {
    assert_invalid(
        parse_str("[routing]\nstage = \"dynamic\"\n", HOST_HEAD),
        "reserved for detonation",
    );
}

#[test]
fn rejects_routing_on_guest() {
    let head = HOST_HEAD.replace("type = \"host\"", "type = \"guest\"");
    assert_invalid(
        parse_str("[routing]\naccepts = \"archive/zip\"\n", &head),
        "only valid for host plugins",
    );
}

#[test]
fn rejects_bad_accepts_regex() {
    assert_invalid(
        parse_str("[routing]\naccepts = \"(\"\n", HOST_HEAD),
        "routing.accepts",
    );
}

#[test]
fn rejects_parameter_type_mismatch() {
    assert_invalid(
        parse_str(
            "[parameters.n]\ntype = \"int\"\ndefault = \"x\"\n",
            HOST_HEAD,
        ),
        "parameter n: default does not match type",
    );
}

#[test]
fn parses_auto_collect_rules() {
    let rt = runtime_with_rules("artifacts");
    let resolved = ResolvedRuntimeConfig::from_raw(&rt);
    resolved.validate(PluginTypeConfig::Guest).unwrap();
    let rules = &resolved.auto_collect_artifacts.rules;
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].include, vec!["drops/**"]);
    assert_eq!(
        rules[0].relation,
        malbox_plugin_transport::artifact::ArtifactRelation::Dropped
    );
    assert!(rules[0].analyze);
}

#[test]
fn rejects_rules_on_external_logs() {
    let rt = runtime_with_rules("external_logs");
    let err = ResolvedRuntimeConfig::from_raw(&rt)
        .validate(PluginTypeConfig::Guest)
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("rules are only supported for auto_collect.artifacts")
    );
}

fn runtime_with_rules(section: &str) -> RuntimeConfig {
    toml::from_str(&format!(
        r#"
state = "ephemeral"
execution = "exclusive"

[[auto_collect.{section}.rules]]
include = ["drops/**"]
relation = "dropped"
"#
    ))
    .expect("valid runtime toml")
}
