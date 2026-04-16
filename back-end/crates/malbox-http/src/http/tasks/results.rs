use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use malbox_database::repositories::task_results::{
    ResultFormat, fetch_task_result, fetch_task_results,
};
use serde::Serialize;

use super::super::AppState;

#[derive(Serialize)]
struct TaskResultResponse {
    id: i32,
    task_id: i32,
    plugin_name: String,
    result_name: String,
    format: String,
    size_bytes: i64,
    file_path: String,
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
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "result not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let stored = std::path::Path::new(&result.file_path);
    let abs_path = if stored.is_absolute() {
        stored.to_path_buf()
    } else {
        state.config.paths.data_dir.join(stored)
    };

    let bytes = match tokio::fs::read(&abs_path).await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("failed to read result file: {e}"),
                    "path": abs_path.display().to_string(),
                })),
            )
                .into_response();
        }
    };

    let content_type = match result.format {
        ResultFormat::Json => "application/json",
        ResultFormat::Bytes => "application/octet-stream",
    };

    let disposition = format!(
        "inline; filename=\"{}.{}\"",
        result.result_name,
        match result.format {
            ResultFormat::Json => "json",
            ResultFormat::Bytes => "bin",
        }
    );

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response()
}

async fn get_task_results(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    match fetch_task_results(&state.pool, id).await {
        Ok(results) => {
            let response: Vec<TaskResultResponse> = results
                .into_iter()
                .map(|r| TaskResultResponse {
                    id: r.id,
                    task_id: r.task_id,
                    plugin_name: r.plugin_name,
                    result_name: r.result_name,
                    format: format!("{:?}", r.format).to_lowercase(),
                    size_bytes: r.size_bytes,
                    file_path: r.file_path,
                    created_on: r.created_on.to_string(),
                })
                .collect();
            (StatusCode::OK, Json(serde_json::json!(response))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
