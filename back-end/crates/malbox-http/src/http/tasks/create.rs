use crate::http::{AppState, Result, error::Error};
use axum::body::Bytes;
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    routing::post,
};
use axum_macros::debug_handler;
use axum_typed_multipart::{FieldData, TryFromField, TryFromMultipart, TypedMultipart};
use magic::cookie::DatabasePaths;
use malbox_database::repositories::{
    machinery::MachinePlatform,
    samples::{Sample, SampleEntity, insert_sample},
    tasks::{Task, TaskState, insert_task},
};
use malbox_utils::hashing::*;
use time::{OffsetDateTime, PrimitiveDateTime};
use tracing::info;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks/create/file", post(create_task_from_file))
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
    ssdeep: String,
}

#[derive(TryFromMultipart)]
struct CreateTaskRequest {
    #[form_data(limit = "unlimited")]
    file: FieldData<Bytes>,
    package: Option<String>,
    module: Option<String>,
    timeout: Option<i64>,
    priority: Option<i64>,
    options: Option<String>,
    machine: Option<String>, // needs to be checked via typed struct or conditions instead of String
    platform: Option<String>,
    tags: Option<String>,
    custom: Option<String>,
    owner: Option<String>,
    memory: Option<bool>,
    unique: Option<bool>,
    enforce_timeout: Option<bool>,
    /// Override the sample filename used on the guest VM.
    /// If not set, the original uploaded filename is used.
    target_filename: Option<String>,
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

    let task_id = task.id.expect("Task must have an ID");
    tracing::Span::current().record("task_id", task_id);

    // Send task to scheduler for processing
    state
        .task_tx
        .send(task.clone())
        .await
        .map_err(|e| Error::Internal(format!("Failed to send task to scheduler: {}", e)))?;

    info!(task_id, "Task submitted to scheduler");

    Ok(Json(TaskResponse {
        task_id: task.id.unwrap(),
    }))
}

fn get_file_info(
    file: &FieldData<Bytes>,
) -> std::result::Result<FileInfo, Box<dyn std::error::Error + Send + Sync>> {
    let file_type = {
        let cookie = magic::Cookie::open(magic::cookie::Flags::default())
            .map_err(|e| format!("Failed to open magic cookie: {}", e))?;
        let cookie = cookie.load(&DatabasePaths::default()).unwrap();
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
        ssdeep: "not-available".to_string(),
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

    Ok(insert_sample(&state.pool, sample).await.unwrap())
}

async fn create_task(
    state: &AppState,
    request: &CreateTaskRequest,
    file_info: &FileInfo,
    sample_id: i64,
) -> Result<Task> {
    let utc_now = OffsetDateTime::now_utc();
    let current_primitive_datetime = PrimitiveDateTime::new(utc_now.date(), utc_now.time());

    let task = Task {
        id: None,
        target: request
            .target_filename
            .clone()
            .unwrap_or_else(|| file_info.name.to_string()),
        timeout: request.timeout.unwrap_or(300),
        priority: request.priority.unwrap_or(1),
        platform: match request.platform.as_deref() {
            Some("windows") => MachinePlatform::Windows,
            Some("linux") => MachinePlatform::Linux,
            _ => MachinePlatform::Windows, // default to Windows for malware analysis
        },
        tags: request
            .tags
            .clone()
            .map(|tags_str| tags_str.split(',').map(|s| s.trim().to_string()).collect()),
        owner: request.owner.clone(),
        enforce_timeout: Some(request.enforce_timeout.unwrap_or(false)),
        created_on: current_primitive_datetime,
        started_on: None,
        completed_on: None,
        status: TaskState::Pending,
        sample_id: Some(sample_id),
        machine_cpus: None,
        machine_id: None,
        machine_memory: None,
        plugins: vec!["0".to_string()],
        profile: None,
    };

    Ok(insert_task(&state.pool, task).await.unwrap())
}
