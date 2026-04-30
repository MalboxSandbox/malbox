use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_database::PgPool;
use malbox_database::repositories::samples::{SampleEntity, fetch_sample_by_id};
use malbox_database::repositories::tasks::{Task, fetch_all_tasks, fetch_task};
use serde::Serialize;

use super::super::AppState;

/// File metadata for a submitted sample. Populated from the `samples` table
/// for file-based submissions; `None` for URL/hash submissions.
#[derive(Serialize)]
pub(super) struct SampleInfo {
    pub(super) file_size: i64,
    pub(super) file_type: String,
    pub(super) md5: String,
    pub(super) crc32: String,
    pub(super) sha1: String,
    pub(super) sha256: String,
    pub(super) sha512: String,
    pub(super) ssdeep: String,
}

impl From<SampleEntity> for SampleInfo {
    fn from(s: SampleEntity) -> Self {
        SampleInfo {
            file_size: s.file_size,
            file_type: s.file_type,
            md5: s.md5,
            crc32: s.crc32,
            sha1: s.sha1,
            sha256: s.sha256,
            sha512: s.sha512,
            ssdeep: s.ssdeep,
        }
    }
}

#[derive(Serialize)]
pub(super) struct TaskResponse {
    pub(super) id: i32,
    pub(super) status: String,
    pub(super) target: String,
    pub(super) platform: String,
    pub(super) timeout: i64,
    pub(super) priority: i64,
    pub(super) owner: Option<String>,
    pub(super) machine_id: Option<i32>,
    pub(super) tags: Option<Vec<String>>,
    pub(super) created_on: String,
    pub(super) completed_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sample: Option<SampleInfo>,
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
            sample: None,
        }
    }
}

/// Build a [`TaskResponse`] with its associated [`SampleInfo`] resolved from
/// the DB when the task has a `sample_id`. A failed or missing sample
/// lookup silently leaves `sample = None` — we don't want a missing sample
/// row to fail the whole task fetch.
pub(super) async fn build_task_response(pool: &PgPool, task: Task) -> TaskResponse {
    let sample_id = task.sample_id;
    let mut response = TaskResponse::from(task);
    if let Some(sid) = sample_id
        && let Ok(Some(sample)) = fetch_sample_by_id(pool, sid).await
    {
        response.sample = Some(SampleInfo::from(sample));
    }
    response
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
        Ok(Some(task)) => {
            let response = build_task_response(&state.pool, task).await;
            (StatusCode::OK, Json(serde_json::json!(response))).into_response()
        }
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
