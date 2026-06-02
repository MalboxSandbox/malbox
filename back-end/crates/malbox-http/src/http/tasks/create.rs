use crate::http::dto::SampleDto;
use crate::http::{AppState, Result, error::Error};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRequest, Request, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::post};
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
        .route("/v1/tasks", post(create_task))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 10000000))
}

/// Created-task response. The FE reads `task_id` and `sample.sha256`.
#[derive(serde::Serialize)]
struct CreateTaskResponse {
    task_id: i32,
    sample: SampleDto,
}

fn split_csv(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Multipart upload form.
#[derive(TryFromMultipart)]
struct UploadFields {
    #[form_data(limit = "unlimited")]
    file: FieldData<Bytes>,
    timeout: Option<i64>,
    priority: Option<i64>,
    platform: Option<String>,
    tags: Option<String>,
    owner: Option<String>,
    target_filename: Option<String>,
    plugins: Option<String>,
    snapshot_id: Option<String>,
}

/// `tags`/`plugins` accept either a CSV string or an array.
#[derive(Deserialize)]
#[serde(untagged)]
enum StringList {
    Csv(String),
    List(Vec<String>),
}

impl StringList {
    fn into_vec(self) -> Vec<String> {
        match self {
            StringList::Csv(s) => split_csv(&s),
            StringList::List(v) => v
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
// `Url`/`Hash` payloads are deserialized to preserve the request contract but
// not yet consumed (ingestion is unimplemented); they are rejected at runtime.
#[allow(dead_code)]
enum TaskSource {
    SampleId(i64),
    Url(String),
    Hash(String),
}

#[derive(Deserialize)]
struct ReferenceBody {
    source: TaskSource,
    timeout: Option<i64>,
    priority: Option<i64>,
    platform: Option<String>,
    tags: Option<StringList>,
    owner: Option<String>,
    target_filename: Option<String>,
    plugins: Option<StringList>,
    snapshot_id: Option<String>,
}

/// One handler, two body encodings, selected by `Content-Type`.
enum CreateTaskBody {
    Upload(UploadFields),
    Reference(ReferenceBody),
}

impl<S> FromRequest<S> for CreateTaskBody
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> std::result::Result<Self, Self::Rejection> {
        let content_type = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();

        if content_type.starts_with("multipart/form-data") {
            let TypedMultipart(fields) = TypedMultipart::<UploadFields>::from_request(req, state)
                .await
                .map_err(|e| e.into_response())?;
            Ok(CreateTaskBody::Upload(fields))
        } else if content_type.starts_with("application/json") {
            let Json(body) = Json::<ReferenceBody>::from_request(req, state)
                .await
                .map_err(|e| e.into_response())?;
            Ok(CreateTaskBody::Reference(body))
        } else {
            Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(serde_json::json!({
                    "error": "expected multipart/form-data or application/json"
                })),
            )
                .into_response())
        }
    }
}

fn parse_platform(s: Option<&str>) -> Option<MachinePlatform> {
    match s {
        Some("linux") => Some(MachinePlatform::Linux),
        Some("windows") => Some(MachinePlatform::Windows),
        Some(_) => Some(MachinePlatform::Windows),
        None => None,
    }
}

struct FileInfo {
    name: String,
    size: i64,
    file_type: String,
    md5: String,
    sha1: String,
    sha256: String,
    sha512: String,
    crc32: String,
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
        md5: get_md5(&file.contents),
        sha1: get_sha1(&file.contents),
        sha256: get_sha256(&file.contents),
        sha512: get_sha512(&file.contents),
        crc32: get_crc32(&file.contents),
    })
}

fn now_primitive() -> PrimitiveDateTime {
    let utc = OffsetDateTime::now_utc();
    PrimitiveDateTime::new(utc.date(), utc.time())
}

async fn dispatch(state: &AppState, task: Task, sample: SampleEntity) -> Result<Response> {
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

    Ok((
        StatusCode::CREATED,
        Json(CreateTaskResponse {
            task_id,
            sample: SampleDto::from(sample),
        }),
    )
        .into_response())
}

async fn create_from_upload(state: &AppState, request: UploadFields) -> Result<Response> {
    let file_info = get_file_info(&request.file)
        .map_err(|e| Error::Internal(format!("Failed to get file information: {}", e)))?;

    state
        .sample_store
        .store(&file_info.sha256, &request.file.contents)
        .await
        .map_err(|e| Error::Internal(format!("Failed to store sample file: {}", e)))?;

    let sample = insert_sample(
        &state.pool,
        Sample {
            file_size: file_info.size,
            file_type: file_info.file_type.clone(),
            md5: file_info.md5.clone(),
            crc32: file_info.crc32.clone(),
            sha1: file_info.sha1.clone(),
            sha256: file_info.sha256.clone(),
            sha512: file_info.sha512.clone(),
            ssdeep: "not-available".to_string(),
        },
    )
    .await
    .map_err(|e| Error::Internal(format!("Failed to insert sample: {}", e)))?;

    let task = Task {
        id: None,
        target: request
            .target_filename
            .clone()
            .unwrap_or_else(|| file_info.name.clone()),
        timeout: request.timeout.unwrap_or(300),
        priority: request.priority.unwrap_or(1),
        platform: parse_platform(request.platform.as_deref()),
        tags: request.tags.as_deref().map(split_csv),
        owner: request.owner.clone(),
        enforce_timeout: Some(true),
        created_on: now_primitive(),
        started_on: None,
        completed_on: None,
        status: TaskState::Pending,
        sample_id: Some(sample.id),
        machine_cpus: None,
        machine_id: None,
        machine_memory: None,
        plugins: request
            .plugins
            .as_deref()
            .map(split_csv)
            .unwrap_or_default(),
        profile: None,
        snapshot_id: request
            .snapshot_id
            .as_deref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok()),
    };
    let task = insert_task(&state.pool, task)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert task: {}", e)))?;

    dispatch(state, task, sample).await
}

async fn create_from_sample(
    state: &AppState,
    sample_id: i64,
    body: ReferenceBody,
) -> Result<Response> {
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

    let task = Task {
        id: None,
        target: body
            .target_filename
            .unwrap_or_else(|| sample.sha256.clone()),
        timeout: body.timeout.unwrap_or(300),
        priority: body.priority.unwrap_or(1),
        platform: parse_platform(body.platform.as_deref()),
        tags: body.tags.map(StringList::into_vec),
        owner: body.owner,
        enforce_timeout: Some(true),
        created_on: now_primitive(),
        started_on: None,
        completed_on: None,
        status: TaskState::Pending,
        sample_id: Some(sample.id),
        machine_cpus: None,
        machine_id: None,
        machine_memory: None,
        plugins: body.plugins.map(StringList::into_vec).unwrap_or_default(),
        profile: None,
        snapshot_id: body
            .snapshot_id
            .as_deref()
            .and_then(|s| uuid::Uuid::parse_str(s).ok()),
    };
    let task = insert_task(&state.pool, task)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert task: {}", e)))?;

    dispatch(state, task, sample).await
}

#[tracing::instrument(skip_all, fields(task_id = tracing::field::Empty))]
async fn create_task(State(state): State<AppState>, body: CreateTaskBody) -> Result<Response> {
    match body {
        CreateTaskBody::Upload(fields) => create_from_upload(&state, fields).await,
        CreateTaskBody::Reference(body) => match body.source {
            TaskSource::SampleId(id) => create_from_sample(&state, id, body).await,
            TaskSource::Url(_) | TaskSource::Hash(_) => Err(Error::NotImplemented(
                "url/hash ingestion not implemented".to_string(),
            )),
        },
    }
}
