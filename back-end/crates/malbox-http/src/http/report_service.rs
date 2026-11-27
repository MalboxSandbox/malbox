use std::collections::BTreeSet;
use std::path::Path;

use malbox_database::PgPool;
use malbox_database::repositories::{
    artifacts::{ArtifactEdge, TaskArtifact, fetch_edges_from_job},
    jobs::{Job, fetch_task_jobs},
    plugin_reports::{
        DbClassification, PluginReport, fetch_plugin_report, fetch_plugin_reports_for_artifact,
    },
};
use malbox_plugin_sdk::report::{
    ArtifactRef, Block, Indicator, PluginInfo, Report, SCHEMA_VERSION, Section, Ttp,
};
use serde::Serialize;

use crate::http::dto::{Aggregate, PluginReportView};

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct ArtifactLink {
    pub result_name: String,
    pub format: String,
    pub size_bytes: i64,
    pub url: String,
    pub artifact_id: i64,
    pub analyze: bool,
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

/// Read a plugin's full report JSON. `report_path` is relative to the data dir.
pub async fn load_full_report(data_dir: &Path, report_path: &str) -> Option<Report> {
    let stored = Path::new(report_path);
    let path = if stored.is_absolute() {
        stored.to_path_buf()
    } else {
        data_dir.join(stored)
    };
    let bytes = tokio::fs::read(&path).await.ok()?;
    serde_json::from_slice::<Report>(&bytes).ok()
}

fn edge_display_name(edge: &ArtifactEdge, child: &TaskArtifact) -> String {
    edge.name.clone().unwrap_or_else(|| child.sha256.clone())
}

fn is_json(child: &TaskArtifact) -> bool {
    child.mime == "application/json"
}

pub fn synthesize_report(plugin_name: &str, artifacts: &[(ArtifactEdge, TaskArtifact)]) -> Report {
    let sections = artifacts
        .iter()
        .map(|(edge, child)| {
            let name = edge_display_name(edge, child);
            let block = if is_json(child) {
                Block::Json {
                    data: serde_json::json!({ "$ref": name }),
                    collapsed: true,
                }
            } else {
                Block::Download {
                    artifact: name.clone(),
                    label: format!("Download {}", name),
                }
            };
            Section {
                id: name.clone(),
                title: name,
                blocks: vec![block],
            }
        })
        .collect();

    let artifact_refs: Vec<ArtifactRef> = artifacts
        .iter()
        .map(|(edge, child)| ArtifactRef::new(edge_display_name(edge, child), "other"))
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
// Artifact links and plugin views
// ---------------------------------------------------------------------------

/// The edges `plugin_name` emitted, keeping only those with stored content.
pub fn plugin_edges(
    edges: &[(ArtifactEdge, TaskArtifact)],
    plugin_name: &str,
) -> Vec<(ArtifactEdge, TaskArtifact)> {
    edges
        .iter()
        .filter(|(e, c)| {
            e.source_plugin.as_deref() == Some(plugin_name) && c.storage_path.is_some()
        })
        .cloned()
        .collect()
}

pub fn artifact_links(task_id: i32, edges: &[(ArtifactEdge, TaskArtifact)]) -> Vec<ArtifactLink> {
    edges
        .iter()
        .map(|(edge, child)| ArtifactLink {
            result_name: edge_display_name(edge, child),
            format: if is_json(child) { "json" } else { "bytes" }.to_string(),
            size_bytes: child.size_bytes,
            url: format!("/v1/tasks/{}/artifacts/{}/content", task_id, child.id),
            artifact_id: child.id,
            analyze: child.analyze,
        })
        .collect()
}

fn job_failed(job: &Job) -> bool {
    matches!(job.state.as_str(), "failed" | "canceled" | "skipped")
}

fn job_finished(job: &Job) -> bool {
    !matches!(job.state.as_str(), "blocked" | "ready" | "running")
}

/// Whether `job` failed to produce `plugin_name`'s report: a guest plugin of a
/// detonation fails once the detonation finishes without its report, any
/// other plugin only when its job failed.
fn job_failed_for_plugin(job: &Job, plugin_name: &str) -> bool {
    if job.kind == "detonation" && job.analyzer != plugin_name {
        job_finished(job)
    } else {
        job_failed(job)
    }
}

/// Plugin views for one artifact, derived from its jobs: one view per host
/// job, and per detonation job one view per guest plugin with a report plus,
/// once the job has finished, a failed view for every guest plugin it ran
/// without producing one.
pub async fn plugin_views_for_artifact(
    pool: &PgPool,
    task_id: i32,
    artifact_id: i64,
) -> Result<Vec<PluginReportView>, malbox_database::error::DatabaseError> {
    let jobs: Vec<Job> = fetch_task_jobs(pool, task_id)
        .await?
        .into_iter()
        .filter(|j| j.artifact_id == artifact_id)
        .collect();
    let reports = fetch_plugin_reports_for_artifact(pool, artifact_id).await?;
    let mut views = Vec::new();

    for job in &jobs {
        let edges = fetch_edges_from_job(pool, job.id).await?;
        let view_for = |name: &str, report: Option<&PluginReport>, failed: bool| PluginReportView {
            plugin_name: name.to_string(),
            report: report.map(build_plugin_report_envelope),
            synthesized: false,
            failed,
            artifacts: artifact_links(task_id, &plugin_edges(&edges, name)),
            source_task_id: None,
        };
        if job.kind == "detonation" {
            let mut reported: BTreeSet<&str> = BTreeSet::new();
            for r in reports.iter().filter(|r| r.job_id == Some(job.id)) {
                reported.insert(r.plugin_name.as_str());
                views.push(view_for(&r.plugin_name, Some(r), false));
            }
            let finished = job_finished(job);
            for name in job.guest_plugins.iter().flatten() {
                if !reported.contains(name.as_str()) {
                    views.push(view_for(name, None, finished));
                }
            }
        } else {
            let report = reports.iter().find(|r| r.plugin_name == job.analyzer);
            views.push(view_for(&job.analyzer, report, job_failed(job)));
        }
    }
    Ok(views)
}

/// Single-plugin view for one artifact: the full report (or a synthesized one)
/// plus the plugin's emitted artifacts. `None` when the plugin never ran on it.
pub async fn single_plugin_view(
    pool: &PgPool,
    data_dir: &Path,
    task_id: i32,
    artifact_id: i64,
    plugin_name: &str,
) -> Result<Option<SinglePluginView>, malbox_database::error::DatabaseError> {
    let jobs: Vec<Job> = fetch_task_jobs(pool, task_id)
        .await?
        .into_iter()
        .filter(|j| {
            j.artifact_id == artifact_id
                && (j.analyzer == plugin_name
                    || (j.kind == "detonation"
                        && j.guest_plugins.iter().flatten().any(|p| p == plugin_name)))
        })
        .collect();
    let row = fetch_plugin_report(pool, artifact_id, plugin_name).await?;
    if jobs.is_empty() && row.is_none() {
        return Ok(None);
    }

    let mut edges = Vec::new();
    for job in &jobs {
        edges.extend(plugin_edges(
            &fetch_edges_from_job(pool, job.id).await?,
            plugin_name,
        ));
    }
    let artifacts = artifact_links(task_id, &edges);

    let view = match row {
        Some(r) => {
            let (report, synthesized) = match load_full_report(data_dir, &r.report_path).await {
                Some(rep) => (rep, false),
                None => (synthesize_report(plugin_name, &edges), true),
            };
            SinglePluginView {
                report: Some(serde_json::to_value(report).unwrap_or(serde_json::Value::Null)),
                synthesized,
                failed: false,
                artifacts,
            }
        }
        None if jobs.iter().all(|j| job_failed_for_plugin(j, plugin_name)) => SinglePluginView {
            report: None,
            synthesized: false,
            failed: true,
            artifacts,
        },
        None => SinglePluginView {
            report: Some(
                serde_json::to_value(synthesize_report(plugin_name, &edges))
                    .unwrap_or(serde_json::Value::Null),
            ),
            synthesized: true,
            failed: false,
            artifacts,
        },
    };
    Ok(Some(view))
}

/// Body of a single-plugin report, shared by the task and sample surfaces.
pub struct SinglePluginView {
    pub report: Option<serde_json::Value>,
    pub synthesized: bool,
    pub failed: bool,
    pub artifacts: Vec<ArtifactLink>,
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
            artifact_id: 1,
            job_id: None,
            report_path: String::new(),
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
        fn pair(
            id: i64,
            name: &str,
            mime: &str,
            plugin: &str,
            stored: bool,
        ) -> (ArtifactEdge, TaskArtifact) {
            let now = OffsetDateTime::now_utc();
            (
                ArtifactEdge {
                    task_id: 42,
                    parent_artifact_id: 1,
                    child_artifact_id: id,
                    relation: "output".into(),
                    source_job_id: Some(7),
                    source_plugin: Some(plugin.into()),
                    name: Some(name.into()),
                    created_on: now,
                },
                TaskArtifact {
                    id,
                    task_id: 42,
                    sha256: format!("{id:064}"),
                    size_bytes: 10,
                    storage_path: stored.then(|| format!("samples/{id}")),
                    analyze: false,
                    sample_id: None,
                    depth: 1,
                    artifact_type: "unknown".into(),
                    mime: mime.into(),
                    state: "stored".into(),
                    drop_reason: None,
                    priority: 1,
                    deadline: None,
                    canceled_by: None,
                    promoted_task_id: None,
                    dynamic_recursion_bypass: false,
                    analyze_requested: false,
                    created_on: now,
                },
            )
        }
        let all = vec![
            pair(2, "matches.json", "application/json", "x", true),
            pair(3, "dump.bin", "application/octet-stream", "x", true),
            pair(4, "other.bin", "application/octet-stream", "y", true),
            pair(5, "refused.bin", "application/octet-stream", "x", false),
        ];
        let artifacts = plugin_edges(&all, "x");
        assert_eq!(artifacts.len(), 2);
        let links = artifact_links(42, &artifacts);
        assert_eq!(links[0].format, "json");
        assert_eq!(links[1].url, "/v1/tasks/42/artifacts/3/content");
        let r = synthesize_report("x", &artifacts);
        assert_eq!(r.plugin.id, "x");
        assert_eq!(r.sections.len(), 2);
        assert!(matches!(r.sections[0].blocks[0], Block::Json { .. }));
        assert!(matches!(r.sections[1].blocks[0], Block::Download { .. }));
        assert_eq!(r.artifacts.len(), 2);
        assert_eq!(r.schema_version, SCHEMA_VERSION);
    }
}
