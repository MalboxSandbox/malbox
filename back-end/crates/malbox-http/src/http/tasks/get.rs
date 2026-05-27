use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use malbox_database::PgPool;
use malbox_database::repositories::machinery::MachinePlatform;
use malbox_database::repositories::samples::{SampleEntity, fetch_sample_by_id};
use malbox_database::repositories::tasks::{
    Task, TaskFilter, TaskState, count_tasks, fetch_task, fetch_tasks_page,
};
use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;
use time::format_description::well_known::Iso8601;

use super::super::AppState;

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
    pub(super) plugins: Vec<String>,
    pub(super) tags: Option<Vec<String>>,
    pub(super) created_on: String,
    pub(super) started_on: Option<String>,
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
            platform: t
                .platform
                .map(|p| format!("{:?}", p).to_lowercase())
                .unwrap_or_default(),
            timeout: t.timeout,
            priority: t.priority,
            owner: t.owner,
            machine_id: t.machine_id,
            plugins: t.plugins,
            tags: t.tags,
            created_on: t.created_on.to_string(),
            started_on: t.started_on.map(|d| d.to_string()),
            completed_on: t.completed_on.map(|d| d.to_string()),
            sample: None,
        }
    }
}

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

#[derive(Deserialize)]
struct ListParams {
    status: Option<String>,
    platform: Option<String>,
    owner: Option<String>,
    tag: Option<String>,
    after: Option<String>,
    before: Option<String>,
    cursor: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
struct PaginatedResponse {
    items: Vec<TaskResponse>,
    next_cursor: Option<String>,
    has_more: bool,
}

#[derive(Serialize, Deserialize)]
struct Cursor {
    ts: String,
    id: i32,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks", get(list_tasks))
        .route("/v1/tasks/count", get(get_task_count))
        .route("/v1/tasks/{id}", get(get_task))
}

fn parse_statuses(csv: &str) -> Vec<TaskState> {
    csv.split(',')
        .filter_map(|s| match s.trim() {
            "pending" => Some(TaskState::Pending),
            "initializing" => Some(TaskState::Initializing),
            "preparing_resources" => Some(TaskState::PreparingResources),
            "running" => Some(TaskState::Running),
            "stopping" => Some(TaskState::Stopping),
            "completed" => Some(TaskState::Completed),
            "failed" => Some(TaskState::Failed),
            "canceled" => Some(TaskState::Canceled),
            _ => None,
        })
        .collect()
}

fn parse_platform(s: &str) -> Option<MachinePlatform> {
    match s {
        "windows" => Some(MachinePlatform::Windows),
        "linux" => Some(MachinePlatform::Linux),
        _ => None,
    }
}

fn parse_iso_datetime(s: &str) -> Option<PrimitiveDateTime> {
    PrimitiveDateTime::parse(s, &Iso8601::DEFAULT).ok()
}

fn decode_cursor(encoded: &str) -> Option<Cursor> {
    let bytes = URL_SAFE_NO_PAD.decode(encoded).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn encode_cursor(task: &Task) -> String {
    let cursor = Cursor {
        ts: task.created_on.to_string(),
        id: task.id.unwrap_or_default(),
    };
    let json = serde_json::to_vec(&cursor).unwrap_or_default();
    URL_SAFE_NO_PAD.encode(&json)
}

fn build_filter(params: &ListParams) -> TaskFilter {
    let statuses = params
        .status
        .as_deref()
        .map(parse_statuses)
        .filter(|v| !v.is_empty());

    let platform = params.platform.as_deref().and_then(parse_platform);

    let (cursor_created_on, cursor_id) = params
        .cursor
        .as_deref()
        .and_then(decode_cursor)
        .map(|c| (parse_iso_datetime(&c.ts), Some(c.id)))
        .unwrap_or((None, None));

    TaskFilter {
        statuses,
        platform,
        owner: params.owner.clone(),
        tag: params.tag.clone(),
        after: params.after.as_deref().and_then(parse_iso_datetime),
        before: params.before.as_deref().and_then(parse_iso_datetime),
        cursor_created_on,
        cursor_id,
        limit: params.limit.unwrap_or(50),
    }
}

async fn list_tasks(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let filter = build_filter(&params);

    match fetch_tasks_page(&state.pool, &filter).await {
        Ok(page) => {
            let next_cursor = if page.has_more {
                page.items.last().map(encode_cursor)
            } else {
                None
            };

            let items: Vec<TaskResponse> = page.items.into_iter().map(TaskResponse::from).collect();

            let response = PaginatedResponse {
                items,
                next_cursor,
                has_more: page.has_more,
            };

            (StatusCode::OK, Json(serde_json::json!(response))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Serialize)]
struct CountResponse {
    total: i64,
    by_status: std::collections::BTreeMap<String, i64>,
}

async fn get_task_count(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let filter = build_filter(&params);

    match count_tasks(&state.pool, &filter).await {
        Ok(counts) => {
            let by_status: std::collections::BTreeMap<String, i64> =
                counts.by_status.into_iter().collect();

            let response = CountResponse {
                total: counts.total,
                by_status,
            };

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
