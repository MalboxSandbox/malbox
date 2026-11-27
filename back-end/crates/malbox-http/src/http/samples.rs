use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use malbox_database::repositories::{
    artifacts::fetch_sample_appearances,
    plugin_reports::{
        fetch_latest_plugin_report_for_sample, fetch_latest_plugin_reports_for_sample,
    },
    sample_verdicts::fetch_sample_verdict_by_sha256,
    samples::fetch_sample_by_hash,
    tasks::{Task, fetch_tasks_by_sample_id, fetch_tasks_for_sample},
};
use serde::{Deserialize, Serialize};

use super::AppState;
use crate::http::dto::{
    PluginReportView, ReportBody, SampleDto, SampleReportResponse, SinglePluginReportResponse,
};
use crate::http::report_service::{
    aggregate_plugin_reports, build_plugin_report_envelope, single_plugin_view,
};
use crate::http::{Result, error::Error};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/samples", get(list_samples))
        .route("/v1/samples/{sha256}", get(get_sample_overview))
        .route("/v1/samples/{sha256}/report", get(get_sample_report))
        .route(
            "/v1/samples/{sha256}/report/plugins/{plugin_name}",
            get(get_sample_plugin_report),
        )
}

#[derive(Deserialize)]
struct ListParams {
    md5: Option<String>,
    sha1: Option<String>,
    sha256: Option<String>,
    sha512: Option<String>,
}

#[derive(Serialize)]
struct TaskSummary {
    id: i32,
    status: String,
    platform: String,
    plugins: Vec<String>,
    timeout: i64,
    owner: Option<String>,
    tags: Option<Vec<String>>,
    created_on: String,
    started_on: Option<String>,
    completed_on: Option<String>,
    partial: bool,
    error_count: i32,
    artifact_count: i32,
}

impl From<Task> for TaskSummary {
    fn from(t: Task) -> Self {
        TaskSummary {
            id: t.id,
            status: t.status.as_str().to_string(),
            platform: t
                .platform
                .map(|p| format!("{:?}", p).to_lowercase())
                .unwrap_or_default(),
            plugins: t.plugins,
            timeout: t.timeout,
            owner: t.owner,
            tags: t.tags,
            created_on: t.created_on.to_string(),
            started_on: t.started_on.map(|d| d.to_string()),
            completed_on: t.completed_on.map(|d| d.to_string()),
            partial: t.partial,
            error_count: t.error_count,
            artifact_count: t.artifact_count,
        }
    }
}

/// Where the sample was found inside another task's artifact tree.
#[derive(Serialize)]
struct SampleAppearance {
    task_id: i32,
    artifact_id: i64,
    depth: i32,
}

#[derive(Serialize)]
struct SampleLookupResponse {
    sample: SampleDto,
    task_ids: Vec<i32>,
}

#[derive(Serialize, Default)]
struct SampleVerdictView {
    worst_verdict: Option<String>,
    worst_score: Option<u8>,
    indicator_count: i32,
    ttp_count: i32,
    plugin_names: Vec<String>,
    task_count: i32,
}

#[derive(Serialize)]
struct SampleOverviewResponse {
    sample: SampleDto,
    tasks: Vec<TaskSummary>,
    aggregate: SampleVerdictView,
    appearances: Vec<SampleAppearance>,
}

// GET /v1/samples?{md5|sha1|sha256|sha512}=hex
async fn list_samples(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<SampleLookupResponse>> {
    let (hash_type, hash_value) = if let Some(v) = &params.sha256 {
        ("sha256", v.as_str())
    } else if let Some(v) = &params.sha1 {
        ("sha1", v.as_str())
    } else if let Some(v) = &params.md5 {
        ("md5", v.as_str())
    } else if let Some(v) = &params.sha512 {
        ("sha512", v.as_str())
    } else {
        return Err(Error::BadRequest(
            "provide one of: sha256, sha1, md5, sha512".to_string(),
        ));
    };

    let sample = fetch_sample_by_hash(&state.pool, hash_type, hash_value)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;
    let task_ids = fetch_tasks_by_sample_id(&state.pool, sample.id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;

    Ok(Json(SampleLookupResponse {
        sample: SampleDto::from(sample),
        task_ids,
    }))
}

// GET /v1/samples/{sha256} - overview from the sample_verdicts cache
async fn get_sample_overview(
    State(state): State<AppState>,
    Path(sha256): Path<String>,
) -> Result<Json<SampleOverviewResponse>> {
    let sample = fetch_sample_by_hash(&state.pool, "sha256", &sha256)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;
    let verdict = fetch_sample_verdict_by_sha256(&state.pool, &sha256)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let tasks = fetch_tasks_for_sample(&state.pool, sample.id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let appearances = fetch_sample_appearances(&state.pool, sample.id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .into_iter()
        .map(|(task_id, artifact_id, depth)| SampleAppearance {
            task_id,
            artifact_id,
            depth,
        })
        .collect();

    let aggregate = match verdict {
        Some(v) => SampleVerdictView {
            worst_verdict: v.classification.map(|c| c.label().to_string()),
            worst_score: v.score.map(|s| s as u8),
            indicator_count: v.indicator_count,
            ttp_count: v.ttp_count,
            plugin_names: v.plugin_names,
            task_count: v.task_count,
        },
        None => SampleVerdictView::default(),
    };

    Ok(Json(SampleOverviewResponse {
        appearances,
        sample: SampleDto::from(sample),
        tasks: tasks.into_iter().map(TaskSummary::from).collect(),
        aggregate,
    }))
}

// GET /v1/samples/{sha256}/report - per-plugin-latest rollup
async fn get_sample_report(
    State(state): State<AppState>,
    Path(sha256): Path<String>,
) -> Result<Json<SampleReportResponse>> {
    let sample = fetch_sample_by_hash(&state.pool, "sha256", &sha256)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;

    let reports = fetch_latest_plugin_reports_for_sample(&state.pool, sample.id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;

    // Per-plugin views (borrowed pass) before `aggregate_plugin_reports` consumes `reports`.
    let plugins: Vec<PluginReportView> = reports
        .iter()
        .map(|r| PluginReportView {
            plugin_name: r.plugin_name.clone(),
            report: Some(build_plugin_report_envelope(r)),
            synthesized: false,
            failed: false,
            artifacts: vec![],
            source_task_id: Some(r.task_id),
        })
        .collect();

    let plugin_count = plugins.len() as u32;
    let aggregate = aggregate_plugin_reports(reports, plugin_count);

    Ok(Json(SampleReportResponse {
        sample: SampleDto::from(sample),
        body: ReportBody { aggregate, plugins },
    }))
}

// GET /v1/samples/{sha256}/report/plugins/{plugin_name}
async fn get_sample_plugin_report(
    State(state): State<AppState>,
    Path((sha256, plugin_name)): Path<(String, String)>,
) -> Result<Json<SinglePluginReportResponse>> {
    let sample = fetch_sample_by_hash(&state.pool, "sha256", &sha256)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;

    let (pr, task_id) = fetch_latest_plugin_report_for_sample(&state.pool, sample.id, &plugin_name)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;

    let view = single_plugin_view(
        &state.pool,
        &state.config.paths.data_dir,
        task_id,
        pr.artifact_id,
        &plugin_name,
    )
    .await
    .map_err(|e| Error::Internal(e.to_string()))?
    .ok_or(Error::NotFound)?;

    Ok(Json(SinglePluginReportResponse {
        plugin_name,
        task_id,
        report: view.report,
        synthesized: view.synthesized,
        failed: view.failed,
        artifacts: view.artifacts,
    }))
}
