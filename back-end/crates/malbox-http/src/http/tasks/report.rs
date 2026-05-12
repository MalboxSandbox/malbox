//! `GET /v1/tasks/{id}/report` — the unified, frontend-renderable view of a
//! task's outputs.
//!
//! Returns one [`PluginReportView`] per plugin that contributed results.
//! If a plugin produced a structured report envelope (a `PluginResult::Json`
//! with `result_name == REPORT_RESULT_NAME`, tagged `role = 'report'` by the
//! scheduler), we deserialize and return it verbatim. Otherwise we synthesize
//! a minimal envelope so the frontend contract is uniform.
//!
//! An `aggregate` section rolls up cross-plugin state (worst verdict wins,
//! deduped indicators and TTPs) so the task summary page can render without
//! re-reading each plugin's envelope.

use std::collections::{BTreeMap, BTreeSet};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_database::repositories::{
    task_results::{ResultFormat, ResultRole, TaskResult, fetch_task_results},
    tasks::fetch_task,
};
use malbox_plugin_sdk::report::{
    ArtifactRef, Block, Classification, Indicator, PluginInfo, Report, SCHEMA_VERSION, Section,
    Ttp, Verdict,
};
use serde::Serialize;

use super::super::AppState;
use super::get::{TaskResponse, build_task_response};
use super::resolve_result_path;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/tasks/{id}/report", get(get_task_report))
}

#[derive(Serialize)]
struct TaskReportResponse {
    task: TaskResponse,
    aggregate: AggregateView,
    plugins: Vec<PluginReportView>,
}

#[derive(Serialize, Default)]
struct AggregateView {
    verdict: Option<String>,
    score: Option<u8>,
    classifications: BTreeMap<String, u32>,
    indicators: Vec<Indicator>,
    ttps: Vec<Ttp>,
    plugin_count: u32,
    report_count: u32,
}

#[derive(Serialize)]
struct PluginReportView {
    plugin_name: String,
    /// Parsed or synthesized envelope. `None` only when parsing a real
    /// report file fails AND there's no artifact to fall back on.
    report: Option<serde_json::Value>,
    /// `true` when the envelope was assembled from raw outputs rather than
    /// a plugin-authored `report` row.
    synthesized: bool,
    artifacts: Vec<ArtifactLink>,
}

#[derive(Serialize)]
struct ArtifactLink {
    result_name: String,
    format: String,
    size_bytes: i64,
    url: String,
}

async fn get_task_report(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> axum::response::Response {
    // 1. Task must exist.
    let task = match fetch_task(&state.pool, task_id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("Task {} not found", task_id)})),
            )
                .into_response();
        }
        Err(e) => return internal_error(e.to_string()),
    };

    // 2. All result rows for this task.
    let rows = match fetch_task_results(&state.pool, task_id).await {
        Ok(r) => r,
        Err(e) => return internal_error(e.to_string()),
    };

    // 3. Group by plugin_name (stable order via BTreeMap).
    let mut by_plugin: BTreeMap<String, Vec<TaskResult>> = BTreeMap::new();
    for row in rows {
        by_plugin
            .entry(row.plugin_name.clone())
            .or_default()
            .push(row);
    }

    // 4. Build one view per plugin.
    let mut plugin_views = Vec::with_capacity(by_plugin.len());
    for (plugin_name, rows) in by_plugin {
        plugin_views.push(build_plugin_view(&state, task_id, plugin_name, rows).await);
    }

    // 5. Aggregate across plugins.
    let aggregate = aggregate_across(&plugin_views);

    let response = TaskReportResponse {
        task: build_task_response(&state.pool, task).await,
        aggregate,
        plugins: plugin_views,
    };

    (StatusCode::OK, Json(serde_json::json!(response))).into_response()
}

async fn build_plugin_view(
    state: &AppState,
    task_id: i32,
    plugin_name: String,
    rows: Vec<TaskResult>,
) -> PluginReportView {
    // Partition report row(s) from artifact rows. If a plugin mistakenly
    // produced multiple report rows, use the first and treat the rest as
    // artifacts — better than losing data.
    let mut report_row: Option<TaskResult> = None;
    let mut artifact_rows: Vec<TaskResult> = Vec::new();
    for row in rows {
        if matches!(row.role, ResultRole::Report) && report_row.is_none() {
            report_row = Some(row);
        } else {
            artifact_rows.push(row);
        }
    }

    let artifacts: Vec<ArtifactLink> = artifact_rows
        .iter()
        .map(|r| ArtifactLink {
            result_name: r.result_name.clone(),
            format: format_name(r.format),
            size_bytes: r.size_bytes,
            url: format!("/v1/tasks/{}/results/{}/content", task_id, r.id),
        })
        .collect();

    let (report_json, synthesized) = match report_row {
        Some(row) => match load_and_parse_report(state, &row).await {
            Some(v) => (Some(v), false),
            // Parse/read failed — fall through to synthesis so the frontend
            // still gets a usable envelope for this plugin.
            None => (Some(synthesize(&plugin_name, &artifact_rows)), true),
        },
        None => (Some(synthesize(&plugin_name, &artifact_rows)), true),
    };

    PluginReportView {
        plugin_name,
        report: report_json,
        synthesized,
        artifacts,
    }
}

async fn load_and_parse_report(state: &AppState, row: &TaskResult) -> Option<serde_json::Value> {
    let path = resolve_result_path(&state.config, row);
    let bytes = tokio::fs::read(&path).await.ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
}

/// Build a minimal `Report` envelope for a plugin that didn't produce one.
/// Each artifact becomes a section with either a JSON block (for JSON outputs)
/// or a Download block (for binary outputs).
fn synthesize(plugin_name: &str, artifacts: &[TaskResult]) -> serde_json::Value {
    let sections = artifacts
        .iter()
        .map(|a| {
            let block = match a.format {
                ResultFormat::Json => Block::Json {
                    // We don't read artifact JSON here — the client can fetch
                    // it via the content URL. For a compact synthesized view,
                    // point to the artifact instead of inlining.
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

    let report = Report {
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
    };

    serde_json::to_value(report).unwrap_or(serde_json::Value::Null)
}

fn aggregate_across(plugins: &[PluginReportView]) -> AggregateView {
    let mut out = AggregateView {
        plugin_count: plugins.len() as u32,
        ..Default::default()
    };

    let mut worst: Option<Classification> = None;
    let mut max_score: Option<u8> = None;
    let mut seen_iocs: BTreeSet<(String, String)> = BTreeSet::new();
    let mut seen_ttps: BTreeSet<String> = BTreeSet::new();

    for pv in plugins {
        let Some(v) = pv.report.as_ref() else {
            continue;
        };
        // Parse the envelope back into a typed `Report` for folding. Unknown
        // or extra fields are ignored (serde default behavior for our types).
        let Ok(rep) = serde_json::from_value::<Report>(v.clone()) else {
            continue;
        };
        if !pv.synthesized {
            out.report_count += 1;
        }

        if let Some(Verdict {
            classification,
            score,
            ..
        }) = rep.verdict
        {
            let key = classification_label(classification);
            *out.classifications.entry(key).or_insert(0) += 1;
            worst = Some(match worst {
                Some(w) if w.severity() >= classification.severity() => w,
                _ => classification,
            });
            if let Some(s) = score {
                max_score = Some(max_score.map_or(s, |m| m.max(s)));
            }
        }

        for ind in rep.indicators {
            if seen_iocs.insert((ind.kind.clone(), ind.value.clone())) {
                out.indicators.push(ind);
            }
        }
        for ttp in rep.ttps {
            if seen_ttps.insert(ttp.id.clone()) {
                out.ttps.push(ttp);
            }
        }
    }

    out.verdict = worst.map(classification_label);
    out.score = max_score;
    out
}

fn classification_label(c: Classification) -> String {
    match c {
        Classification::Clean => "clean",
        Classification::Suspicious => "suspicious",
        Classification::Malicious => "malicious",
        Classification::Unknown => "unknown",
    }
    .to_string()
}

fn format_name(f: ResultFormat) -> String {
    match f {
        ResultFormat::Json => "json",
        ResultFormat::Bytes => "bytes",
    }
    .to_string()
}

fn internal_error(msg: String) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": msg})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use malbox_plugin_sdk::report::{Confidence, ReportBuilder};
    use serde_json::json;

    /// Helper: build a `PluginReportView` holding a pre-built report.
    fn pv(plugin: &str, report: Report, synthesized: bool) -> PluginReportView {
        PluginReportView {
            plugin_name: plugin.into(),
            report: Some(serde_json::to_value(report).unwrap()),
            synthesized,
            artifacts: vec![],
        }
    }

    #[test]
    fn aggregate_picks_worst_verdict() {
        let a = ReportBuilder::new("a", "1")
            .verdict(Classification::Clean, Some(10), None)
            .build();
        let b = ReportBuilder::new("b", "1")
            .verdict(Classification::Malicious, Some(80), Some(Confidence::High))
            .build();
        let c = ReportBuilder::new("c", "1")
            .verdict(Classification::Suspicious, Some(50), None)
            .build();

        let agg = aggregate_across(&[pv("a", a, false), pv("b", b, false), pv("c", c, false)]);
        assert_eq!(agg.verdict.as_deref(), Some("malicious"));
        assert_eq!(agg.score, Some(80));
        assert_eq!(agg.classifications.get("malicious"), Some(&1));
        assert_eq!(agg.classifications.get("clean"), Some(&1));
        assert_eq!(agg.classifications.get("suspicious"), Some(&1));
        assert_eq!(agg.report_count, 3);
    }

    #[test]
    fn aggregate_dedupes_indicators_and_ttps() {
        let a = ReportBuilder::new("a", "1")
            .indicator(Indicator::new("sha256", "deadbeef"))
            .indicator(Indicator::new("ipv4", "1.2.3.4"))
            .ttp(Ttp::new("T1055", "Process Injection"))
            .build();
        let b = ReportBuilder::new("b", "1")
            .indicator(Indicator::new("sha256", "deadbeef")) // dup
            .indicator(Indicator::new("domain", "evil.tld"))
            .ttp(Ttp::new("T1055", "Process Injection (dup)")) // dup by id
            .ttp(Ttp::new("T1027", "Obfuscated Files"))
            .build();

        let agg = aggregate_across(&[pv("a", a, false), pv("b", b, false)]);
        // 3 unique IOCs: sha256/deadbeef, ipv4/1.2.3.4, domain/evil.tld
        assert_eq!(agg.indicators.len(), 3);
        // 2 unique TTPs
        assert_eq!(agg.ttps.len(), 2);
        let ttp_ids: BTreeSet<_> = agg.ttps.iter().map(|t| t.id.as_str()).collect();
        assert!(ttp_ids.contains("T1055"));
        assert!(ttp_ids.contains("T1027"));
    }

    #[test]
    fn aggregate_handles_no_verdicts() {
        let a = ReportBuilder::new("a", "1").build();
        let agg = aggregate_across(&[pv("a", a, true)]);
        assert!(agg.verdict.is_none());
        assert!(agg.score.is_none());
        assert_eq!(agg.report_count, 0); // synthesized doesn't count
        assert_eq!(agg.plugin_count, 1);
    }

    #[test]
    fn synthesize_produces_valid_envelope() {
        use time::OffsetDateTime;
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
        let v = synthesize("x", &artifacts);
        // Envelope round-trips into a Report.
        let r: Report = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(r.plugin.id, "x");
        assert_eq!(r.sections.len(), 2);
        // JSON artifact → json block referencing the artifact name.
        assert!(matches!(r.sections[0].blocks[0], Block::Json { .. }));
        // Bytes artifact → download block.
        assert!(matches!(r.sections[1].blocks[0], Block::Download { .. }));
        // Artifact refs enumerated.
        assert_eq!(r.artifacts.len(), 2);
        assert_eq!(v["schema_version"], json!(SCHEMA_VERSION));
    }
}
