use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use malbox_database::repositories::{
    artifacts::{TaskArtifact, fetch_artifact, fetch_root_artifact},
    plugin_reports::{fetch_plugin_reports_for_artifact, fetch_plugin_reports_for_task},
    tasks::fetch_task,
};
use serde::Deserialize;
use std::collections::BTreeSet;

use super::super::AppState;
use crate::http::dto::{
    ReportBody, SinglePluginReportResponse, TaskReportResponse, attach_one_task,
};
use crate::http::report_service::{
    aggregate_plugin_reports, plugin_views_for_artifact, single_plugin_view,
};
use crate::http::{Result, error::Error};

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks/{id}/report", get(get_task_report))
        .route(
            "/v1/tasks/{id}/report/plugins/{plugin_name}",
            get(get_plugin_report),
        )
}

#[derive(Deserialize)]
struct ArtifactQuery {
    artifact_id: Option<i64>,
}

/// The requested artifact of `task_id`, or its root when none is given.
async fn resolve_artifact(
    state: &AppState,
    task_id: i32,
    artifact_id: Option<i64>,
) -> Result<Option<TaskArtifact>> {
    let artifact = match artifact_id {
        Some(id) => {
            let a = fetch_artifact(&state.pool, id)
                .await
                .map_err(|e| Error::Internal(e.to_string()))?
                .ok_or(Error::NotFound)?;
            if a.task_id != task_id {
                return Err(Error::NotFound);
            }
            Some(a)
        }
        None => fetch_root_artifact(&state.pool, task_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?,
    };
    Ok(artifact)
}

async fn get_task_report(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
    Query(q): Query<ArtifactQuery>,
) -> Result<Json<TaskReportResponse>> {
    let task = fetch_task(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;

    let artifact = resolve_artifact(&state, task_id, q.artifact_id).await?;
    let plugins = match &artifact {
        Some(a) => plugin_views_for_artifact(&state.pool, task_id, a.id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?,
        None => Vec::new(),
    };

    // With an artifact: that artifact only. Without: the whole tree.
    let reports = match (q.artifact_id, &artifact) {
        (Some(_), Some(a)) => fetch_plugin_reports_for_artifact(&state.pool, a.id).await,
        _ => fetch_plugin_reports_for_task(&state.pool, task_id).await,
    }
    .map_err(|e| Error::Internal(e.to_string()))?;

    let plugin_names: BTreeSet<&str> = plugins
        .iter()
        .map(|v| v.plugin_name.as_str())
        .chain(reports.iter().map(|r| r.plugin_name.as_str()))
        .collect();
    let plugin_count = u32::try_from(plugin_names.len()).unwrap_or(u32::MAX);
    let aggregate = aggregate_plugin_reports(reports, plugin_count);
    let task_dto = attach_one_task(&state.pool, task).await;

    Ok(Json(TaskReportResponse {
        task: task_dto,
        body: ReportBody { aggregate, plugins },
    }))
}

async fn get_plugin_report(
    State(state): State<AppState>,
    Path((task_id, plugin_name)): Path<(i32, String)>,
    Query(q): Query<ArtifactQuery>,
) -> Result<Json<SinglePluginReportResponse>> {
    fetch_task(&state.pool, task_id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;
    let artifact = resolve_artifact(&state, task_id, q.artifact_id)
        .await?
        .ok_or(Error::NotFound)?;

    let view = single_plugin_view(
        &state.pool,
        &state.config.paths.data_dir,
        task_id,
        artifact.id,
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
