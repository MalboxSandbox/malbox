use crate::http::{AppState, Result, error::Error};
use axum::body::Bytes;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    routing::post,
};
use axum_macros::debug_handler;
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use magic::cookie::DatabasePaths;
use malbox_database::repositories::{
    machinery::MachinePlatform,
    samples::{Sample, SampleEntity, fetch_sample_by_id, insert_sample},
    tasks::{Task, TaskState, insert_task},
};
use malbox_utils::hashing::*;
use serde::Deserialize;
use time::{OffsetDateTime, PrimitiveDateTime};
use tracing::info;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks/create/file", post(create_task_from_file))
        .route(
            "/v1/tasks/create/sample/{sample_id}",
            post(create_task_from_sample),
        )
        .layer(DefaultBodyLimit::max(1024 * 1024 * 10000000))
}

#[derive(serde::Serialize)]
struct TaskResponse {
    task_id: i32,
}

#[derive(Debug)]
struct FileInfo {
    name: String,
    size: i64,
    file_type: String,
    md5: String,
    sha1: String,
    sha256: String,
    sha512: String,
    crc32: String,
    _ssdeep: String,
}

#[derive(TryFromMultipart)]
struct CreateTaskRequest {
    #[form_data(limit = "unlimited")]
    file: FieldData<Bytes>,
    timeout: Option<i64>,
    priority: Option<i64>,
    platform: Option<String>,
    tags: Option<String>,
    owner: Option<String>,
    /// Override the sample filename used on the guest VM.
    /// If not set, the original uploaded filename is used.
    target_filename: Option<String>,
    /// Comma-separated list of plugin IDs to run for this task.
    plugins: Option<String>,
    /// UUID of the snapshot to use for this task.
    snapshot_id: Option<String>,
}

#[tracing::instrument(skip_all, fields(task_id = tracing::field::Empty), err)]
#[debug_handler]
async fn create_task_from_file(
    State(state): State<AppState>,
    TypedMultipart(request): TypedMultipart<CreateTaskRequest>,
) -> Result<Json<TaskResponse>> {
    let file_info = get_file_info(&request.file)
        .map_err(|e| Error::Internal(format!("Failed to get file information: {}", e)))?;

    state
        .sample_store
        .store(&file_info.sha256, &request.file.contents)
        .await
        .map_err(|e| Error::Internal(format!("Failed to store sample file: {}", e)))?;

    let sample = create_sample(&state, &file_info).await?;
    let task = create_task(&state, &request, &file_info, sample.id).await?;

    let task_id = task
        .id
        .ok_or_else(|| Error::Internal("Task ID not returned from database".into()))?;
    tracing::Span::current().record("task_id", task_id);

    state
        .task_tx
        .send(task)
        .await
        .map_err(|e| Error::Internal(format!("Failed to send task to scheduler: {}", e)))?;

    info!(task_id, "Task submitted to scheduler");

    Ok(Json(TaskResponse { task_id }))
}

fn get_file_info(
    file: &FieldData<Bytes>,
) -> std::result::Result<FileInfo, Box<dyn std::error::Error + Send + Sync>> {
    let file_type = {
        let cookie = magic::Cookie::open(magic::cookie::Flags::default())
            .map_err(|e| format!("Failed to open magic cookie: {}", e))?;
        let cookie = cookie
            .load(&DatabasePaths::default())
            .map_err(|e| format!("Failed to load magic database: {}", e))?;
        cookie
            .buffer(&file.contents)
            .map_err(|e| format!("Failed to analyze file type: {}", e))?
    };

    Ok(FileInfo {
        name: file
            .metadata
            .file_name
            .as_deref()
            .unwrap_or("data.bin")
            .to_string(),
        size: file.contents.len() as i64,
        file_type,
        md5: get_md5(&mut file.contents.to_vec()),
        sha1: get_sha1(&mut file.contents.to_vec()),
        sha256: get_sha256(&mut file.contents.to_vec()),
        sha512: get_sha512(&mut file.contents.to_vec()),
        crc32: get_crc32(&mut file.contents.to_vec()),
        _ssdeep: "not-available".to_string(),
    })
}

async fn create_sample(state: &AppState, file_info: &FileInfo) -> Result<SampleEntity> {
    let sample = Sample {
        file_size: file_info.size,
        file_type: file_info.file_type.clone(),
        md5: file_info.md5.clone(),
        crc32: file_info.crc32.clone(),
        sha1: file_info.sha1.clone(),
        sha256: file_info.sha256.clone(),
        sha512: file_info.sha512.clone(),
        ssdeep: "not-available".to_string(),
    };

    insert_sample(&state.pool, sample)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert sample: {}", e)))
}

async fn create_task(
    state: &AppState,
    request: &CreateTaskRequest,
    file_info: &FileInfo,
    sample_id: i64,
) -> Result<Task> {
    let utc_now = OffsetDateTime::now_utc();
    let current_primitive_datetime = PrimitiveDateTime::new(utc_now.date(), utc_now.time());

    let plugins: Vec<String> = request
        .plugins
        .as_ref()
        .map(|p| {
            p.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let snapshot_id = request
        .snapshot_id
        .as_ref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok());

    let task = Task {
        id: None,
        target: request
            .target_filename
            .clone()
            .unwrap_or_else(|| file_info.name.to_string()),
        timeout: request.timeout.unwrap_or(300),
        priority: request.priority.unwrap_or(1),
        platform: match request.platform.as_deref() {
            Some("windows") => Some(MachinePlatform::Windows),
            Some("linux") => Some(MachinePlatform::Linux),
            Some(_) => Some(MachinePlatform::Windows),
            None => None,
        },
        tags: request
            .tags
            .clone()
            .map(|tags_str| tags_str.split(',').map(|s| s.trim().to_string()).collect()),
        owner: request.owner.clone(),
        enforce_timeout: Some(true),
        created_on: current_primitive_datetime,
        started_on: None,
        completed_on: None,
        status: TaskState::Pending,
        sample_id: Some(sample_id),
        machine_cpus: None,
        machine_id: None,
        machine_memory: None,
        plugins,
        profile: None,
        snapshot_id,
    };

    insert_task(&state.pool, task)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert task: {}", e)))
}

#[derive(Deserialize)]
struct RescanRequest {
    timeout: Option<i64>,
    priority: Option<i64>,
    platform: Option<String>,
    tags: Option<String>,
    owner: Option<String>,
    target_filename: Option<String>,
    plugins: Option<String>,
    snapshot_id: Option<String>,
}

#[tracing::instrument(skip_all, fields(task_id = tracing::field::Empty, sample_id = sample_id), err)]
#[debug_handler]
async fn create_task_from_sample(
    State(state): State<AppState>,
    Path(sample_id): Path<i64>,
    Json(request): Json<RescanRequest>,
) -> Result<Json<TaskResponse>> {
    let sample = fetch_sample_by_id(&state.pool, sample_id)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch sample: {}", e)))?
        .ok_or(Error::NotFound)?;

    if !state
        .sample_store
        .exists(&sample.sha256)
        .await
        .map_err(|e| Error::Internal(format!("Failed to check sample store: {}", e)))?
    {
        return Err(Error::unprocessable_entity([(
            "sample",
            "Sample file no longer exists on disk",
        )]));
    }

    let utc_now = OffsetDateTime::now_utc();
    let current_primitive_datetime = PrimitiveDateTime::new(utc_now.date(), utc_now.time());

    let plugins: Vec<String> = request
        .plugins
        .as_ref()
        .map(|p| {
            p.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let snapshot_id = request
        .snapshot_id
        .as_ref()
        .and_then(|s| uuid::Uuid::parse_str(s).ok());

    let target = request
        .target_filename
        .unwrap_or_else(|| sample.sha256.clone());

    let task = Task {
        id: None,
        target,
        timeout: request.timeout.unwrap_or(300),
        priority: request.priority.unwrap_or(1),
        platform: match request.platform.as_deref() {
            Some("windows") => Some(MachinePlatform::Windows),
            Some("linux") => Some(MachinePlatform::Linux),
            Some(_) => Some(MachinePlatform::Windows),
            None => None,
        },
        tags: request
            .tags
            .map(|tags_str| tags_str.split(',').map(|s| s.trim().to_string()).collect()),
        owner: request.owner,
        enforce_timeout: Some(true),
        created_on: current_primitive_datetime,
        started_on: None,
        completed_on: None,
        status: TaskState::Pending,
        sample_id: Some(sample.id),
        machine_cpus: None,
        machine_id: None,
        machine_memory: None,
        plugins,
        profile: None,
        snapshot_id,
    };

    let task = insert_task(&state.pool, task)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert task: {}", e)))?;

    let task_id = task
        .id
        .ok_or_else(|| Error::Internal("Task ID not returned from database".into()))?;
    tracing::Span::current().record("task_id", task_id);

    state
        .task_tx
        .send(task)
        .await
        .map_err(|e| Error::Internal(format!("Failed to send task to scheduler: {}", e)))?;

    info!(task_id, "Rescan task submitted to scheduler");

    Ok(Json(TaskResponse { task_id }))
}
