use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};
use malbox_database::repositories::{
    plugin_reports::fetch_plugin_reports_for_task,
    task_results::{ResultRole, fetch_task_results},
    tasks::fetch_task,
};
use std::collections::BTreeSet;

use super::super::AppState;
use crate::http::dto::{
    PluginReportView, ReportBody, SinglePluginReportResponse, TaskReportResponse, attach_one_task,
};
use crate::http::report_service::{
    aggregate_plugin_reports, build_artifact_links, build_plugin_report_envelope,
    flat_artifact_links, is_terminal, load_full_report, synthesize_report,
};
use crate::http::{Result, error::Error};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks/{id}/report", get(get_task_report))
        .route(
            "/v1/tasks/{id}/report/plugins/{plugin_name}",
            get(get_plugin_report),
        )
}

async fn get_task_report(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> Result<Json<TaskReportResponse>> {
    let task = fetch_task(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;

    let reports = fetch_plugin_reports_for_task(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let results = fetch_task_results(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;

    let artifacts_by_plugin = build_artifact_links(task_id, &results);
    let declared = task.plugins.clone();
    let status = task.status.clone();

    // Build per-plugin views (borrowed pass) before `aggregate_plugin_reports`
    // consumes `reports`.
    let mut plugins: Vec<PluginReportView> = reports
        .iter()
        .map(|r| {
            let arts = artifacts_by_plugin
                .get(&r.plugin_name)
                .cloned()
                .unwrap_or_default();
            PluginReportView {
                plugin_name: r.plugin_name.clone(),
                report: Some(build_plugin_report_envelope(r)),
                synthesized: false,
                failed: false,
                artifacts: arts,
                source_task_id: None,
            }
        })
        .collect();

    if is_terminal(&status) {
        let reported: BTreeSet<&str> = reports.iter().map(|r| r.plugin_name.as_str()).collect();
        for name in &declared {
            if !reported.contains(name.as_str()) {
                let arts = artifacts_by_plugin.get(name).cloned().unwrap_or_default();
                plugins.push(PluginReportView {
                    plugin_name: name.clone(),
                    report: None,
                    synthesized: false,
                    failed: true,
                    artifacts: arts,
                    source_task_id: None,
                });
            }
        }
    }

    let aggregate = aggregate_plugin_reports(reports, declared.len() as u32);
    let task_dto = attach_one_task(&state.pool, task).await;

    Ok(Json(TaskReportResponse {
        task: task_dto,
        body: ReportBody { aggregate, plugins },
    }))
}

async fn get_plugin_report(
    State(state): State<AppState>,
    Path((task_id, plugin_name)): Path<(i32, String)>,
) -> Result<Json<SinglePluginReportResponse>> {
    let task = fetch_task(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;

    let results = fetch_task_results(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;

    let plugin_rows: Vec<_> = results
        .into_iter()
        .filter(|r| r.plugin_name == plugin_name)
        .collect();

    if plugin_rows.is_empty() {
        if is_terminal(&task.status) && task.plugins.contains(&plugin_name) {
            return Ok(Json(SinglePluginReportResponse {
                plugin_name,
                task_id,
                report: None,
                synthesized: false,
                failed: true,
                artifacts: vec![],
            }));
        }
        return Err(Error::NotFound);
    }

    let report_row = plugin_rows
        .iter()
        .find(|r| matches!(r.role, ResultRole::Report));
    let artifact_rows: Vec<_> = plugin_rows
        .iter()
        .filter(|r| !matches!(r.role, ResultRole::Report))
        .cloned()
        .collect();

    let (report, synthesized) = match report_row {
        Some(row) => match load_full_report(&state.config, row).await {
            Some(r) => (Some(r), false),
            None => (Some(synthesize_report(&plugin_name, &artifact_rows)), true),
        },
        None => (Some(synthesize_report(&plugin_name, &artifact_rows)), true),
    };
    let report_value = report.map(|r| serde_json::to_value(r).unwrap_or(serde_json::Value::Null));
    let artifacts = flat_artifact_links(task_id, &plugin_rows);

    Ok(Json(SinglePluginReportResponse {
        plugin_name,
        task_id,
        report: report_value,
        synthesized,
        failed: false,
        artifacts,
    }))
}
