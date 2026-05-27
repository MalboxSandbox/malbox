//! Task report endpoints - tiered by response weight.
//!
//! * `GET /v1/tasks/{id}/report/summary` - Lightweight aggregate + per-plugin
//!   metadata. No full report sections returned. Suitable for the task overview.
//! * `GET /v1/tasks/{id}/report` - Full aggregate with indicators/TTPs plus
//!   per-plugin report envelopes (sections stripped for weight). Use for the
//!   combined view.
//! * `GET /v1/tasks/{id}/report/plugins/{name}` - Complete report envelope
//!   for a single plugin including all sections. One disk read.
//! * `GET /v1/tasks/{id}/report/indicators` - Aggregated IOCs across plugins.
//! * `GET /v1/tasks/{id}/report/ttps` - Aggregated TTPs across plugins.

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
    tasks::{TaskState, fetch_task},
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
    Router::new()
        .route("/v1/tasks/{id}/report/summary", get(get_report_summary))
        .route("/v1/tasks/{id}/report", get(get_task_report))
        .route(
            "/v1/tasks/{id}/report/plugins/{plugin_name}",
            get(get_plugin_report),
        )
        .route(
            "/v1/tasks/{id}/report/indicators",
            get(get_report_indicators),
        )
        .route("/v1/tasks/{id}/report/ttps", get(get_report_ttps))
}

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ArtifactLink {
    result_name: String,
    format: String,
    size_bytes: i64,
    url: String,
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

struct ParsedPlugin {
    plugin_name: String,
    report: Option<Report>,
    synthesized: bool,
    failed: bool,
    artifacts: Vec<ArtifactLink>,
}

// ---------------------------------------------------------------------------
// GET /v1/tasks/{id}/report/summary
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct PluginSummary {
    plugin_name: String,
    has_report: bool,
    synthesized: bool,
    failed: bool,
    artifact_count: usize,
}

#[derive(Serialize)]
struct SummaryAggregateView {
    verdict: Option<String>,
    score: Option<u8>,
    classifications: BTreeMap<String, u32>,
    indicator_count: usize,
    ttp_count: usize,
    plugin_count: u32,
    report_count: u32,
}

#[derive(Serialize)]
struct ReportSummaryResponse {
    task: TaskResponse,
    aggregate: SummaryAggregateView,
    plugins: Vec<PluginSummary>,
}

async fn get_report_summary(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> axum::response::Response {
    let (task, parsed) = match load_task_plugins(&state, task_id).await {
        Ok(v) => v,
        Err(r) => return r,
    };

    let agg = aggregate_across(&parsed);

    let plugins: Vec<PluginSummary> = parsed
        .iter()
        .map(|p| PluginSummary {
            plugin_name: p.plugin_name.clone(),
            has_report: p.report.is_some(),
            synthesized: p.synthesized,
            failed: p.failed,
            artifact_count: p.artifacts.len(),
        })
        .collect();

    let summary_agg = SummaryAggregateView {
        verdict: agg.verdict,
        score: agg.score,
        classifications: agg.classifications,
        indicator_count: agg.indicators.len(),
        ttp_count: agg.ttps.len(),
        plugin_count: agg.plugin_count,
        report_count: agg.report_count,
    };

    let response = ReportSummaryResponse {
        task: build_task_response(&state.pool, task).await,
        aggregate: summary_agg,
        plugins,
    };

    (StatusCode::OK, Json(serde_json::json!(response))).into_response()
}

// ---------------------------------------------------------------------------
// GET /v1/tasks/{id}/report (full - but sections stripped from envelopes)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TaskReportResponse {
    task: TaskResponse,
    aggregate: AggregateView,
    plugins: Vec<PluginReportView>,
}

#[derive(Serialize)]
struct PluginReportView {
    plugin_name: String,
    report: Option<serde_json::Value>,
    synthesized: bool,
    failed: bool,
    artifacts: Vec<ArtifactLink>,
}

async fn get_task_report(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> axum::response::Response {
    let (task, parsed) = match load_task_plugins(&state, task_id).await {
        Ok(v) => v,
        Err(r) => return r,
    };

    let aggregate = aggregate_across(&parsed);

    let plugins: Vec<PluginReportView> = parsed
        .into_iter()
        .map(|p| {
            let report_value = p.report.map(|r| {
                let stripped = Report {
                    sections: vec![],
                    ..r
                };
                serde_json::to_value(stripped).unwrap_or(serde_json::Value::Null)
            });
            PluginReportView {
                plugin_name: p.plugin_name,
                report: report_value,
                synthesized: p.synthesized,
                failed: p.failed,
                artifacts: p.artifacts,
            }
        })
        .collect();

    let response = TaskReportResponse {
        task: build_task_response(&state.pool, task).await,
        aggregate,
        plugins,
    };

    (StatusCode::OK, Json(serde_json::json!(response))).into_response()
}

// ---------------------------------------------------------------------------
// GET /v1/tasks/{id}/report/plugins/{plugin_name}
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SinglePluginReportResponse {
    plugin_name: String,
    report: Option<serde_json::Value>,
    synthesized: bool,
    failed: bool,
    artifacts: Vec<ArtifactLink>,
}

async fn get_plugin_report(
    State(state): State<AppState>,
    Path((task_id, plugin_name)): Path<(i32, String)>,
) -> axum::response::Response {
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

    let rows = match fetch_task_results(&state.pool, task_id).await {
        Ok(r) => r,
        Err(e) => return internal_error(e.to_string()),
    };

    let plugin_rows: Vec<TaskResult> = rows
        .into_iter()
        .filter(|r| r.plugin_name == plugin_name)
        .collect();

    if plugin_rows.is_empty() {
        if is_terminal(&task.status) && task.plugins.contains(&plugin_name) {
            let response = SinglePluginReportResponse {
                plugin_name,
                report: None,
                synthesized: false,
                failed: true,
                artifacts: vec![],
            };
            return (StatusCode::OK, Json(serde_json::json!(response))).into_response();
        }

        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("No results for plugin '{}'", plugin_name)})),
        )
            .into_response();
    }

    let parsed = build_parsed_plugin(&state, task_id, plugin_name, plugin_rows).await;

    let report_value = parsed
        .report
        .map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null));

    let response = SinglePluginReportResponse {
        plugin_name: parsed.plugin_name,
        report: report_value,
        synthesized: parsed.synthesized,
        failed: parsed.failed,
        artifacts: parsed.artifacts,
    };

    (StatusCode::OK, Json(serde_json::json!(response))).into_response()
}

// ---------------------------------------------------------------------------
// GET /v1/tasks/{id}/report/indicators
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct IndicatorsResponse {
    indicators: Vec<Indicator>,
    total: usize,
}

async fn get_report_indicators(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> axum::response::Response {
    let (_, parsed) = match load_task_plugins(&state, task_id).await {
        Ok(v) => v,
        Err(r) => return r,
    };

    let agg = aggregate_across(&parsed);
    let total = agg.indicators.len();

    let response = IndicatorsResponse {
        indicators: agg.indicators,
        total,
    };

    (StatusCode::OK, Json(serde_json::json!(response))).into_response()
}

// ---------------------------------------------------------------------------
// GET /v1/tasks/{id}/report/ttps
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TtpsResponse {
    ttps: Vec<Ttp>,
    total: usize,
}

async fn get_report_ttps(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> axum::response::Response {
    let (_, parsed) = match load_task_plugins(&state, task_id).await {
        Ok(v) => v,
        Err(r) => return r,
    };

    let agg = aggregate_across(&parsed);
    let total = agg.ttps.len();

    let response = TtpsResponse {
        ttps: agg.ttps,
        total,
    };

    (StatusCode::OK, Json(serde_json::json!(response))).into_response()
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

async fn load_task_plugins(
    state: &AppState,
    task_id: i32,
) -> std::result::Result<
    (
        malbox_database::repositories::tasks::Task,
        Vec<ParsedPlugin>,
    ),
    axum::response::Response,
> {
    let task = match fetch_task(&state.pool, task_id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("Task {} not found", task_id)})),
            )
                .into_response());
        }
        Err(e) => return Err(internal_error(e.to_string())),
    };

    let rows = match fetch_task_results(&state.pool, task_id).await {
        Ok(r) => r,
        Err(e) => return Err(internal_error(e.to_string())),
    };

    let mut by_plugin: BTreeMap<String, Vec<TaskResult>> = BTreeMap::new();
    for row in rows {
        by_plugin
            .entry(row.plugin_name.clone())
            .or_default()
            .push(row);
    }

    let mut parsed = Vec::with_capacity(by_plugin.len());
    for (plugin_name, rows) in by_plugin {
        parsed.push(build_parsed_plugin(state, task_id, plugin_name, rows).await);
    }

    if is_terminal(&task.status) {
        for name in &task.plugins {
            if !parsed.iter().any(|p| p.plugin_name == *name) {
                parsed.push(ParsedPlugin {
                    plugin_name: name.clone(),
                    report: None,
                    synthesized: false,
                    failed: true,
                    artifacts: vec![],
                });
            }
        }
    }

    Ok((task, parsed))
}

async fn build_parsed_plugin(
    state: &AppState,
    task_id: i32,
    plugin_name: String,
    rows: Vec<TaskResult>,
) -> ParsedPlugin {
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

    let (report, synthesized) = match report_row {
        Some(row) => match load_report(state, &row).await {
            Some(r) => (Some(r), false),
            None => (Some(synthesize_report(&plugin_name, &artifact_rows)), true),
        },
        None => (Some(synthesize_report(&plugin_name, &artifact_rows)), true),
    };

    ParsedPlugin {
        plugin_name,
        report,
        synthesized,
        failed: false,
        artifacts,
    }
}

async fn load_report(state: &AppState, row: &TaskResult) -> Option<Report> {
    let path = resolve_result_path(&state.config, row);
    let bytes = tokio::fs::read(&path).await.ok()?;
    serde_json::from_slice::<Report>(&bytes).ok()
}

fn synthesize_report(plugin_name: &str, artifacts: &[TaskResult]) -> Report {
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

fn aggregate_across(plugins: &[ParsedPlugin]) -> AggregateView {
    let mut out = AggregateView {
        plugin_count: plugins.len() as u32,
        ..Default::default()
    };

    let mut worst: Option<Classification> = None;
    let mut max_score: Option<u8> = None;
    let mut seen_iocs: BTreeSet<(String, String)> = BTreeSet::new();
    let mut seen_ttps: BTreeSet<String> = BTreeSet::new();

    for pv in plugins {
        let Some(rep) = &pv.report else {
            continue;
        };
        if !pv.synthesized {
            out.report_count += 1;
        }

        if let Some(Verdict {
            classification,
            score,
            ..
        }) = &rep.verdict
        {
            let key = classification_label(*classification);
            *out.classifications.entry(key).or_insert(0) += 1;
            worst = Some(match worst {
                Some(w) if w.severity() >= classification.severity() => w,
                _ => *classification,
            });
            if let Some(s) = score {
                max_score = Some(max_score.map_or(*s, |m| m.max(*s)));
            }
        }

        for ind in &rep.indicators {
            if seen_iocs.insert((ind.kind.clone(), ind.value.clone())) {
                out.indicators.push(ind.clone());
            }
        }
        for ttp in &rep.ttps {
            if seen_ttps.insert(ttp.id.clone()) {
                out.ttps.push(ttp.clone());
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

fn is_terminal(s: &TaskState) -> bool {
    matches!(
        s,
        TaskState::Completed | TaskState::Failed | TaskState::Canceled
    )
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

    fn pp(plugin: &str, report: Report, synthesized: bool) -> ParsedPlugin {
        ParsedPlugin {
            plugin_name: plugin.into(),
            report: Some(report),
            synthesized,
            failed: false,
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

        let agg = aggregate_across(&[pp("a", a, false), pp("b", b, false), pp("c", c, false)]);
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

        let agg = aggregate_across(&[pp("a", a, false), pp("b", b, false)]);
        assert_eq!(agg.indicators.len(), 3);
        assert_eq!(agg.ttps.len(), 2);
        let ttp_ids: BTreeSet<_> = agg.ttps.iter().map(|t| t.id.as_str()).collect();
        assert!(ttp_ids.contains("T1055"));
        assert!(ttp_ids.contains("T1027"));
    }

    #[test]
    fn aggregate_handles_no_verdicts() {
        let a = ReportBuilder::new("a", "1").build();
        let agg = aggregate_across(&[pp("a", a, true)]);
        assert!(agg.verdict.is_none());
        assert!(agg.score.is_none());
        assert_eq!(agg.report_count, 0);
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
        let r = synthesize_report("x", &artifacts);
        assert_eq!(r.plugin.id, "x");
        assert_eq!(r.sections.len(), 2);
        assert!(matches!(r.sections[0].blocks[0], Block::Json { .. }));
        assert!(matches!(r.sections[1].blocks[0], Block::Download { .. }));
        assert_eq!(r.artifacts.len(), 2);
        assert_eq!(r.schema_version, SCHEMA_VERSION);
    }
}
