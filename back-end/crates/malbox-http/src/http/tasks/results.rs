use axum::{
    Json, Router,
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use malbox_database::repositories::task_results::{
    ResultFormat, ResultRole, fetch_task_result, fetch_task_results,
};
use serde::Serialize;
use tokio_util::io::ReaderStream;

use super::super::AppState;
use super::resolve_result_path;
use crate::http::report_service::format_name;
use crate::http::{Result, error::Error};

#[derive(Serialize)]
struct TaskResultResponse {
    id: i32,
    task_id: i32,
    plugin_name: String,
    result_name: String,
    format: String,
    role: String,
    size_bytes: i64,
    created_on: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks/{id}/results", get(get_task_results))
        .route(
            "/v1/tasks/{id}/results/{result_id}/content",
            get(get_task_result_content),
        )
}

async fn get_task_result_content(
    State(state): State<AppState>,
    Path((task_id, result_id)): Path<(i32, i32)>,
) -> axum::response::Response {
    let result = match fetch_task_result(&state.pool, result_id).await {
        Ok(Some(r)) if r.task_id == task_id => r,
        Ok(_) => {
            return Error::NotFound.into_response();
        }
        Err(e) => {
            return Error::Internal(e.to_string()).into_response();
        }
    };

    let abs_path = resolve_result_path(&state.config, &result);

    let canonical = match tokio::fs::canonicalize(&abs_path).await {
        Ok(p) => p,
        Err(_) => {
            return Error::Internal("result file unavailable".to_string()).into_response();
        }
    };

    let data_dir = match tokio::fs::canonicalize(&state.config.paths.data_dir).await {
        Ok(p) => p,
        Err(_) => {
            return Error::Internal("result file unavailable".to_string()).into_response();
        }
    };

    if !canonical.starts_with(&data_dir) {
        tracing::warn!(
            result_id,
            path = %canonical.display(),
            "path traversal blocked: result path escapes data directory"
        );
        return Error::Forbidden.into_response();
    }

    let file = match tokio::fs::File::open(&canonical).await {
        Ok(f) => f,
        Err(_) => {
            return Error::Internal("result file unavailable".to_string()).into_response();
        }
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    let content_type = match result.format {
        ResultFormat::Json => "application/json",
        ResultFormat::Bytes => "application/octet-stream",
    };

    let ext = match result.format {
        ResultFormat::Json => "json",
        ResultFormat::Bytes => "bin",
    };
    let disposition = format!("inline; filename=\"{}.{}\"", result.result_name, ext);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_DISPOSITION, disposition),
            (header::CONTENT_LENGTH, result.size_bytes.to_string()),
        ],
        body,
    )
        .into_response()
}

async fn get_task_results(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<Vec<TaskResultResponse>>> {
    let results = fetch_task_results(&state.pool, id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let response: Vec<TaskResultResponse> = results
        .into_iter()
        .map(|r| TaskResultResponse {
            id: r.id,
            task_id: r.task_id,
            plugin_name: r.plugin_name,
            result_name: r.result_name,
            format: format_name(r.format),
            role: role_name(r.role),
            size_bytes: r.size_bytes,
            created_on: r.created_on.to_string(),
        })
        .collect();
    Ok(Json(response))
}

fn role_name(r: ResultRole) -> String {
    match r {
        ResultRole::Report => "report",
        ResultRole::Artifact => "artifact",
    }
    .to_string()
}
