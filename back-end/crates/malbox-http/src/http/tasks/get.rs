use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_database::repositories::tasks::{Task, fetch_all_tasks, fetch_task};
use serde::Serialize;

use super::super::AppState;

#[derive(Serialize)]
struct TaskResponse {
    id: i32,
    status: String,
    target: String,
    platform: String,
    timeout: i64,
    priority: i64,
    owner: Option<String>,
    machine_id: Option<i32>,
    tags: Option<Vec<String>>,
    created_on: String,
    completed_on: Option<String>,
}

impl From<Task> for TaskResponse {
    fn from(t: Task) -> Self {
        TaskResponse {
            id: t.id.unwrap_or_default(),
            status: format!("{:?}", t.status).to_lowercase(),
            target: t.target,
            platform: format!("{:?}", t.platform).to_lowercase(),
            timeout: t.timeout,
            priority: t.priority,
            owner: t.owner,
            machine_id: t.machine_id,
            tags: t.tags,
            created_on: t.created_on.to_string(),
            completed_on: t.completed_on.map(|d| d.to_string()),
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks", get(list_tasks))
        .route("/v1/tasks/{id}", get(get_task))
}

async fn list_tasks(State(state): State<AppState>) -> impl IntoResponse {
    match fetch_all_tasks(&state.pool).await {
        Ok(tasks) => {
            let response: Vec<TaskResponse> = tasks.into_iter().map(TaskResponse::from).collect();
            (StatusCode::OK, Json(serde_json::json!(response))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn get_task(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    match fetch_task(&state.pool, id).await {
        Ok(Some(task)) => (
            StatusCode::OK,
            Json(serde_json::json!(TaskResponse::from(task))),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Task {} not found", id)})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
