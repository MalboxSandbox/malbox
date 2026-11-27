//! Per-submission analysis parameters, stored in `tasks.params`.

use std::collections::BTreeMap;

use malbox_config::OrchestrationConfig;
use malbox_plugin_manifest::ParameterKind;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tracing::warn;

use crate::package::PACKAGES;
use crate::routing::{AnalyzerSpec, RoutingTable};

/// Whether the submitted sample is detonated.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DetonateMode {
    /// Detonate when a profile matches.
    #[default]
    Auto,
    /// Always try; an unmatched root fails as unserviceable.
    Always,
    Never,
}

/// Which children may be detonated.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChildDetonation {
    /// Children of artifacts that were not themselves detonated.
    #[default]
    Extracted,
    None,
    All,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct InitialData {
    pub passwords: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct DetonationOverrides {
    pub profile: Option<String>,
    pub package: Option<String>,
    pub dll_function: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct SubmissionParams {
    pub detonate: DetonateMode,
    pub detonate_children: ChildDetonation,
    pub max_depth: Option<u32>,
    pub max_artifacts: Option<u32>,
    pub max_detonations: Option<u32>,
    pub max_total_bytes: Option<u64>,
    pub max_artifact_time_secs: Option<u64>,
    pub deadline_secs: Option<u64>,
    pub deep_scan: bool,
    pub ignore_cache: bool,
    pub ignore_recursion_prevention: bool,
    pub ignore_filtering: bool,
    pub ignore_size: bool,
    pub plugin_params: BTreeMap<String, BTreeMap<String, Value>>,
    pub initial_data: InitialData,
    pub detonation: DetonationOverrides,
}

fn kind_name(kind: ParameterKind) -> &'static str {
    match kind {
        ParameterKind::Bool => "bool",
        ParameterKind::Int => "int",
        ParameterKind::String => "string",
        ParameterKind::List => "list",
    }
}

fn value_matches(kind: ParameterKind, v: &Value) -> bool {
    match kind {
        ParameterKind::Bool => v.is_boolean(),
        ParameterKind::Int => v.is_i64(),
        ParameterKind::String => v.is_string(),
        ParameterKind::List => v
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string)),
    }
}

impl SubmissionParams {
    /// Parse `tasks.params`. Invalid JSON (impossible for rows written by
    /// `create_task`) falls back to the defaults.
    pub fn from_json(v: &Value) -> Self {
        serde_json::from_value(v.clone()).unwrap_or_else(|e| {
            warn!(error = %e, "invalid stored submission params, using defaults");
            Self::default()
        })
    }

    /// Check overrides against the configured caps and plugin parameters
    /// against the manifests. Returns `(field, message)` pairs.
    pub fn validate(
        &self,
        settings: &OrchestrationConfig,
        routing: &RoutingTable,
    ) -> Result<(), Vec<(String, String)>> {
        let mut errors = Vec::new();
        let caps: [(&str, Option<u64>, u64); 6] = [
            (
                "params.max_depth",
                self.max_depth.map(u64::from),
                u64::from(settings.max_depth),
            ),
            (
                "params.max_artifacts",
                self.max_artifacts.map(u64::from),
                u64::from(settings.max_artifacts),
            ),
            (
                "params.max_detonations",
                self.max_detonations.map(u64::from),
                u64::from(settings.max_detonations),
            ),
            (
                "params.max_total_bytes",
                self.max_total_bytes,
                settings.max_total_bytes,
            ),
            (
                "params.max_artifact_time_secs",
                self.max_artifact_time_secs,
                settings.max_artifact_time_secs,
            ),
            (
                "params.deadline_secs",
                self.deadline_secs,
                settings.task_deadline_secs,
            ),
        ];
        for (field, value, cap) in caps {
            if value.is_some_and(|v| v > cap) {
                errors.push((field.to_string(), format!("must be at most {cap}")));
            }
        }

        for (plugin, values) in &self.plugin_params {
            let Some(spec) = routing.analyzer(plugin) else {
                errors.push((
                    format!("params.plugin_params.{plugin}"),
                    "unknown plugin".to_string(),
                ));
                continue;
            };
            for (key, value) in values {
                let field = format!("params.plugin_params.{plugin}.{key}");
                match spec.parameters.get(key) {
                    None => errors.push((field, "unknown parameter".to_string())),
                    Some(p) if !value_matches(p.kind, value) => {
                        errors.push((field, format!("expected {}", kind_name(p.kind))));
                    }
                    Some(_) => {}
                }
            }
        }

        if let Some(profile) = &self.detonation.profile
            && routing.profile(profile).is_none()
        {
            errors.push((
                "params.detonation.profile".to_string(),
                "unknown detonation profile".to_string(),
            ));
        }
        if let Some(package) = &self.detonation.package
            && !PACKAGES.contains(&package.as_str())
        {
            errors.push((
                "params.detonation.package".to_string(),
                "unknown package".to_string(),
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Manifest defaults overlaid with the submission's values for `spec`.
pub fn resolve_plugin_params(spec: &AnalyzerSpec, params: &SubmissionParams) -> Map<String, Value> {
    let mut out: Map<String, Value> = spec
        .parameters
        .iter()
        .map(|(k, p)| {
            let v = serde_json::to_value(&p.default).unwrap_or(Value::Null);
            (k.clone(), v)
        })
        .collect();
    if let Some(overrides) = params.plugin_params.get(&spec.name) {
        for (k, v) in overrides {
            out.insert(k.clone(), v.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::{DetonationProfile, anchored};
    use malbox_database::repositories::machinery::MachinePlatform;
    use malbox_plugin_manifest::{ParameterConfig, Stage};
    use std::collections::BTreeSet;
    use std::time::Duration;

    fn routing() -> RoutingTable {
        let mut parameters = BTreeMap::new();
        parameters.insert(
            "passwords".to_string(),
            ParameterConfig {
                kind: ParameterKind::List,
                default: toml::Value::Array(vec![]),
                description: None,
            },
        );
        RoutingTable::new(
            vec![AnalyzerSpec {
                name: "zip".into(),
                version: "1.0.0".into(),
                stage: Stage::Extract,
                category: "extract".into(),
                accepts: anchored("archive/zip").unwrap(),
                rejects: None,
                sources_exclude: vec![],
                timeout: Duration::from_secs(60),
                cacheable: true,
                recursion_prevention: vec![],
                uses_indicators: false,
                monitored_keys: vec![],
                parameters,
            }],
            BTreeSet::new(),
            vec![DetonationProfile {
                name: "win".into(),
                accepts: anchored(".*").unwrap(),
                platform: MachinePlatform::Windows,
                arch: None,
                snapshot: None,
                tags: vec![],
                guest_plugins: vec![],
                timeout: Duration::from_secs(60),
            }],
        )
    }

    #[test]
    fn validate_reports_every_problem() {
        let mut p = SubmissionParams {
            max_depth: Some(100),
            ..Default::default()
        };
        p.plugin_params
            .entry("zip".into())
            .or_default()
            .insert("passwords".into(), Value::from(3));
        p.plugin_params
            .entry("zip".into())
            .or_default()
            .insert("bogus".into(), Value::from(true));
        p.plugin_params.entry("nope".into()).or_default();
        p.detonation.profile = Some("missing".into());
        p.detonation.package = Some("pdf".into());
        let errors = p
            .validate(&OrchestrationConfig::default(), &routing())
            .unwrap_err();
        let fields: BTreeSet<_> = errors.iter().map(|(f, _)| f.as_str()).collect();
        assert!(fields.contains("params.max_depth"));
        assert!(fields.contains("params.plugin_params.zip.passwords"));
        assert!(fields.contains("params.plugin_params.zip.bogus"));
        assert!(fields.contains("params.plugin_params.nope"));
        assert!(fields.contains("params.detonation.profile"));
        assert!(fields.contains("params.detonation.package"));
        assert!(errors.contains(&(
            "params.plugin_params.zip.passwords".to_string(),
            "expected list".to_string()
        )));
    }

    #[test]
    fn resolved_params_overlay_defaults() {
        let table = routing();
        let spec = table.analyzer("zip").unwrap();
        let mut p = SubmissionParams::default();
        assert_eq!(
            resolve_plugin_params(spec, &p).get("passwords"),
            Some(&Value::Array(vec![]))
        );
        p.plugin_params
            .entry("zip".into())
            .or_default()
            .insert("passwords".into(), serde_json::json!(["a"]));
        assert_eq!(
            resolve_plugin_params(spec, &p).get("passwords"),
            Some(&serde_json::json!(["a"]))
        );
    }

    #[test]
    fn unknown_fields_rejected() {
        assert!(
            serde_json::from_value::<SubmissionParams>(serde_json::json!({"bogus": 1})).is_err()
        );
        let p: SubmissionParams =
            serde_json::from_value(serde_json::json!({"detonate": "never"})).unwrap();
        assert_eq!(p.detonate, DetonateMode::Never);
    }
}
