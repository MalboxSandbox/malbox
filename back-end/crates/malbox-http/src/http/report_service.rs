use std::collections::{BTreeMap, BTreeSet};

use malbox_config::Config as MalboxConfig;
use malbox_database::repositories::{
    plugin_reports::{DbClassification, PluginReport},
    task_results::{ResultFormat, ResultRole, TaskResult},
    tasks::TaskState,
};
use malbox_plugin_sdk::report::{
    ArtifactRef, Block, Indicator, PluginInfo, Report, SCHEMA_VERSION, Section, Ttp,
};
use serde::Serialize;

use super::tasks::resolve_result_path;
use crate::http::dto::Aggregate;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct ArtifactLink {
    pub result_name: String,
    pub format: String,
    pub size_bytes: i64,
    pub url: String,
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// Fold a set of plugin reports into a cross-plugin aggregate. Consumes the
/// reports so each row's `indicators`/`ttps` JSONB is moved out and parsed once
/// (no clone). `plugin_count` is supplied by the caller (declared plugins for a
/// task; distinct reported plugins for a sample).
pub fn aggregate_plugin_reports(reports: Vec<PluginReport>, plugin_count: u32) -> Aggregate {
    let mut out = Aggregate {
        plugin_count,
        ..Default::default()
    };

    let mut worst: Option<DbClassification> = None;
    let mut max_score: Option<u8> = None;
    let mut seen_iocs: BTreeSet<(String, String)> = BTreeSet::new();
    let mut seen_ttps: BTreeSet<String> = BTreeSet::new();

    for mut r in reports {
        out.report_count += 1;

        if let Some(c) = r.classification {
            *out.classifications
                .entry(c.label().to_string())
                .or_insert(0) += 1;
            worst = Some(match worst {
                Some(w) if w.severity() >= c.severity() => w,
                _ => c,
            });
        }

        if let Some(s) = r.score {
            let s = s as u8;
            max_score = Some(max_score.map_or(s, |m| m.max(s)));
        }

        let indicators = std::mem::take(&mut r.indicators);
        if let Ok(inds) = serde_json::from_value::<Vec<Indicator>>(indicators) {
            for ind in inds {
                if seen_iocs.insert((ind.kind.clone(), ind.value.clone())) {
                    out.indicators.push(ind);
                }
            }
        }

        let ttps = std::mem::take(&mut r.ttps);
        if let Ok(ts) = serde_json::from_value::<Vec<Ttp>>(ttps) {
            for ttp in ts {
                if seen_ttps.insert(ttp.id.clone()) {
                    out.ttps.push(ttp);
                }
            }
        }
    }

    out.verdict = worst.map(|w| w.label().to_string());
    out.score = max_score;
    out
}

pub fn build_plugin_report_envelope(report: &PluginReport) -> serde_json::Value {
    let mut envelope = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "plugin": {
            "id": report.plugin_name,
            "version": report.plugin_version,
        },
    });

    if let Some(dn) = &report.display_name {
        envelope["plugin"]["display_name"] = serde_json::json!(dn);
    }

    if let Some(c) = report.classification {
        let mut verdict = serde_json::json!({
            "classification": c.label(),
        });
        if let Some(s) = report.score {
            verdict["score"] = serde_json::json!(s);
        }
        if let Some(conf) = report.confidence {
            verdict["confidence"] = serde_json::json!(conf.label());
        }
        if !report.labels.is_empty() {
            verdict["labels"] = serde_json::json!(report.labels);
        }
        envelope["verdict"] = verdict;
    }

    if report.indicators != serde_json::json!([]) {
        envelope["indicators"] = report.indicators.clone();
    }
    if report.ttps != serde_json::json!([]) {
        envelope["ttps"] = report.ttps.clone();
    }
    if let Some(s) = &report.summary {
        envelope["summary"] = serde_json::json!(s);
    }

    envelope
}

// ---------------------------------------------------------------------------
// Report loading / synthesis
// ---------------------------------------------------------------------------

pub async fn load_full_report(config: &MalboxConfig, row: &TaskResult) -> Option<Report> {
    let path = resolve_result_path(config, row);
    let bytes = tokio::fs::read(&path).await.ok()?;
    serde_json::from_slice::<Report>(&bytes).ok()
}

pub fn synthesize_report(plugin_name: &str, artifacts: &[TaskResult]) -> Report {
    let sections = artifacts
        .iter()
        .map(|a| {
            let block = match a.format {
                ResultFormat::Json => Block::Json {
                    data: serde_json::json!({ "$ref": a.result_name }),
                    collapsed: true,
                },
                ResultFormat::Bytes => Block::Download {
                    artifact: a.result_name.clone(),
                    label: format!("Download {}", a.result_name),
                },
            };
            Section {
                id: a.result_name.clone(),
                title: a.result_name.clone(),
                blocks: vec![block],
            }
        })
        .collect();

    let artifact_refs: Vec<ArtifactRef> = artifacts
        .iter()
        .map(|a| ArtifactRef::new(a.result_name.clone(), "other"))
        .collect();

    Report {
        schema_version: SCHEMA_VERSION,
        plugin: PluginInfo {
            id: plugin_name.to_string(),
            version: String::new(),
            display_name: None,
        },
        verdict: None,
        indicators: vec![],
        ttps: vec![],
        artifacts: artifact_refs,
        summary: None,
        sections,
        raw: None,
    }
}

// ---------------------------------------------------------------------------
// Artifact helpers
// ---------------------------------------------------------------------------

pub fn build_artifact_links(
    task_id: i32,
    rows: &[TaskResult],
) -> BTreeMap<String, Vec<ArtifactLink>> {
    let mut by_plugin: BTreeMap<String, Vec<ArtifactLink>> = BTreeMap::new();
    for r in rows {
        if matches!(r.role, ResultRole::Report) {
            continue;
        }
        by_plugin
            .entry(r.plugin_name.clone())
            .or_default()
            .push(ArtifactLink {
                result_name: r.result_name.clone(),
                format: format_name(r.format),
                size_bytes: r.size_bytes,
                url: format!("/v1/tasks/{}/results/{}/content", task_id, r.id),
            });
    }
    by_plugin
}

pub fn flat_artifact_links(task_id: i32, rows: &[TaskResult]) -> Vec<ArtifactLink> {
    rows.iter()
        .filter(|r| matches!(r.role, ResultRole::Artifact))
        .map(|r| ArtifactLink {
            result_name: r.result_name.clone(),
            format: format_name(r.format),
            size_bytes: r.size_bytes,
            url: format!("/v1/tasks/{}/results/{}/content", task_id, r.id),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub fn is_terminal(s: &TaskState) -> bool {
    matches!(
        s,
        TaskState::Completed | TaskState::Failed | TaskState::Canceled
    )
}

/// Wire representation of a task status. Explicit snake_case mapping rather than
/// `format!("{:?}", s).to_lowercase()` so multi-word variants serialize with the
/// underscore the front-end expects (e.g. `preparing_resources`, not
/// `preparingresources`). Matches the `task_state` SQL enum and the FE union.
pub fn task_status_str(s: &TaskState) -> String {
    match s {
        TaskState::Pending => "pending",
        TaskState::Initializing => "initializing",
        TaskState::PreparingResources => "preparing_resources",
        TaskState::Running => "running",
        TaskState::Stopping => "stopping",
        TaskState::Completed => "completed",
        TaskState::Failed => "failed",
        TaskState::Canceled => "canceled",
    }
    .to_string()
}

pub fn format_name(f: ResultFormat) -> String {
    match f {
        ResultFormat::Json => "json",
        ResultFormat::Bytes => "bytes",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn make_report(
        plugin: &str,
        classification: Option<DbClassification>,
        score: Option<i16>,
        indicators: Vec<Indicator>,
        ttps: Vec<Ttp>,
    ) -> PluginReport {
        PluginReport {
            id: 0,
            task_id: 1,
            plugin_name: plugin.to_string(),
            display_name: None,
            plugin_version: "1".to_string(),
            classification,
            score,
            confidence: None,
            labels: vec![],
            indicators: serde_json::to_value(&indicators).unwrap(),
            ttps: serde_json::to_value(&ttps).unwrap(),
            summary: None,
            section_count: 0,
            artifact_count: 0,
            created_on: OffsetDateTime::now_utc(),
        }
    }

    #[test]
    fn aggregate_picks_worst_verdict() {
        let reports = vec![
            make_report("a", Some(DbClassification::Clean), Some(10), vec![], vec![]),
            make_report(
                "b",
                Some(DbClassification::Malicious),
                Some(80),
                vec![],
                vec![],
            ),
            make_report(
                "c",
                Some(DbClassification::Suspicious),
                Some(50),
                vec![],
                vec![],
            ),
        ];
        let agg = aggregate_plugin_reports(reports, 3);
        assert_eq!(agg.verdict.as_deref(), Some("malicious"));
        assert_eq!(agg.score, Some(80));
        assert_eq!(agg.classifications.get("malicious"), Some(&1));
        assert_eq!(agg.classifications.get("clean"), Some(&1));
        assert_eq!(agg.classifications.get("suspicious"), Some(&1));
        assert_eq!(agg.report_count, 3);
    }

    #[test]
    fn aggregate_dedupes_indicators_and_ttps() {
        let reports = vec![
            make_report(
                "a",
                None,
                None,
                vec![
                    Indicator::new("sha256", "deadbeef"),
                    Indicator::new("ipv4", "1.2.3.4"),
                ],
                vec![Ttp::new("T1055", "Process Injection")],
            ),
            make_report(
                "b",
                None,
                None,
                vec![
                    Indicator::new("sha256", "deadbeef"), // dup
                    Indicator::new("domain", "evil.tld"),
                ],
                vec![
                    Ttp::new("T1055", "Process Injection (dup)"), // dup by id
                    Ttp::new("T1027", "Obfuscated Files"),
                ],
            ),
        ];
        let agg = aggregate_plugin_reports(reports, 2);
        assert_eq!(agg.indicators.len(), 3);
        assert_eq!(agg.ttps.len(), 2);
        let ttp_ids: BTreeSet<_> = agg.ttps.iter().map(|t| t.id.as_str()).collect();
        assert!(ttp_ids.contains("T1055"));
        assert!(ttp_ids.contains("T1027"));
    }

    #[test]
    fn aggregate_handles_no_verdicts() {
        let reports = vec![make_report("a", None, None, vec![], vec![])];
        let agg = aggregate_plugin_reports(reports, 1);
        assert!(agg.verdict.is_none());
        assert!(agg.score.is_none());
        assert_eq!(agg.report_count, 1);
        assert_eq!(agg.plugin_count, 1);
    }

    #[test]
    fn severity_ordering() {
        assert!(DbClassification::Clean.severity() < DbClassification::Unknown.severity());
        assert!(DbClassification::Unknown.severity() < DbClassification::Suspicious.severity());
        assert!(DbClassification::Suspicious.severity() < DbClassification::Malicious.severity());
    }

    #[test]
    fn synthesize_produces_valid_envelope() {
        let artifacts = vec![
            TaskResult {
                id: 1,
                task_id: 42,
                plugin_name: "x".into(),
                result_name: "matches.json".into(),
                format: ResultFormat::Json,
                role: ResultRole::Artifact,
                size_bytes: 10,
                file_path: "/x".into(),
                created_on: OffsetDateTime::now_utc(),
            },
            TaskResult {
                id: 2,
                task_id: 42,
                plugin_name: "x".into(),
                result_name: "dump.bin".into(),
                format: ResultFormat::Bytes,
                role: ResultRole::Artifact,
                size_bytes: 100,
                file_path: "/y".into(),
                created_on: OffsetDateTime::now_utc(),
            },
        ];
        let r = synthesize_report("x", &artifacts);
        assert_eq!(r.plugin.id, "x");
        assert_eq!(r.sections.len(), 2);
        assert!(matches!(r.sections[0].blocks[0], Block::Json { .. }));
        assert!(matches!(r.sections[1].blocks[0], Block::Download { .. }));
        assert_eq!(r.artifacts.len(), 2);
        assert_eq!(r.schema_version, SCHEMA_VERSION);
    }
}
