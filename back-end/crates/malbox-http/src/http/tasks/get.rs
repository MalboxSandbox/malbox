use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use malbox_database::PgPool;
use malbox_database::repositories::machinery::MachinePlatform;
use malbox_database::repositories::samples::fetch_samples_by_ids;
use malbox_database::repositories::tasks::{
    Task, TaskFilter, TaskState, count_tasks, fetch_task, fetch_tasks_page,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::PrimitiveDateTime;
use time::format_description::well_known::Iso8601;

use super::super::AppState;
use crate::http::Result;
use crate::http::dto::{SampleDto, TaskDto};
use crate::http::error::Error;

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
    items: Vec<TaskDto>,
    next_cursor: Option<String>,
    has_more: bool,
}

/// Opaque keyset cursor. `ts` is serialized via `time`'s default serde
/// representation (a component array) so it round-trips exactly; the cursor is
/// base64'd JSON and never parsed as a human-readable string.
#[derive(Serialize, Deserialize)]
struct Cursor {
    ts: PrimitiveDateTime,
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
        ts: task.created_on,
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

/// Attach each task's sample via a single batched query (no N+1).
async fn attach_samples(pool: &PgPool, tasks: Vec<Task>) -> Vec<TaskDto> {
    let ids: Vec<i64> = tasks.iter().filter_map(|t| t.sample_id).collect();
    let mut by_id: HashMap<i64, SampleDto> = HashMap::new();
    if !ids.is_empty()
        && let Ok(samples) = fetch_samples_by_ids(pool, &ids).await
    {
        for s in samples {
            by_id.insert(s.id, SampleDto::from(s));
        }
    }
    tasks
        .into_iter()
        .map(|t| {
            let sample_id = t.sample_id;
            let mut dto = TaskDto::from(t);
            if let Some(sid) = sample_id {
                dto.sample = by_id.get(&sid).cloned();
            }
            dto
        })
        .collect()
}

async fn list_tasks(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<PaginatedResponse>> {
    let filter = build_filter(&params);
    let page = fetch_tasks_page(&state.pool, &filter)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let next_cursor = if page.has_more {
        page.items.last().map(encode_cursor)
    } else {
        None
    };
    let items = attach_samples(&state.pool, page.items).await;
    Ok(Json(PaginatedResponse {
        items,
        next_cursor,
        has_more: page.has_more,
    }))
}

#[derive(Serialize)]
struct CountResponse {
    total: i64,
    by_status: std::collections::BTreeMap<String, i64>,
}

async fn get_task_count(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<CountResponse>> {
    let filter = build_filter(&params);
    let counts = count_tasks(&state.pool, &filter)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?;
    let by_status: std::collections::BTreeMap<String, i64> = counts.by_status.into_iter().collect();
    Ok(Json(CountResponse {
        total: counts.total,
        by_status,
    }))
}

async fn get_task(State(state): State<AppState>, Path(id): Path<i32>) -> Result<Json<TaskDto>> {
    let task = fetch_task(&state.pool, id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;
    let mut dtos = attach_samples(&state.pool, vec![task]).await;
    Ok(Json(dtos.remove(0)))
}
