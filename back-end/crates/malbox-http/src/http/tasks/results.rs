use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_database::repositories::task_results::fetch_task_results;
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
    Router::new().route("/v1/tasks/{id}/results", get(get_task_results))
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
