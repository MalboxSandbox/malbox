use crate::http::dto::SampleDto;
use crate::http::{AppState, Result, error::Error};
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRequest, Request, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router, routing::post};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use malbox_database::repositories::{
    artifacts::TaskArtifact,
    machinery::{MachinePlatform, fetch_machine},
    samples::{Sample, SampleEntity, fetch_sample_by_id, insert_sample},
    tasks::Task,
};
use malbox_scheduler::{NewSubmission, SubmissionParams, create_task as orchestrator_create_task};
use malbox_utils::hashing::{get_crc32, get_md5, get_sha1, get_sha256, get_sha512};
use malbox_utils::identify::{Identification, identify};
use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;
use tracing::info;

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/tasks", post(create_task))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 10_000_000))
}

/// Created-task response. The FE reads `task_id` and `sample.sha256`.
#[derive(serde::Serialize)]
pub(super) struct CreateTaskResponse {
    pub task_id: i32,
    pub root_artifact_id: i64,
    pub sample: SampleDto,
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
    excluded_plugins: Option<String>,
    snapshot_id: Option<String>,
    machine_id: Option<i32>,
    /// JSON-encoded `SubmissionParams`.
    params: Option<String>,
}

/// `tags`/`plugins` accept either a CSV string or an array.
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum StringList {
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
#[expect(
    dead_code,
    reason = "Url/Hash payloads are parsed but ingestion is unimplemented"
)]
enum TaskSource {
    SampleId(i64),
    Url(String),
    Hash(String),
}

/// Submission options shared by the JSON reference body and the promote body.
#[derive(Deserialize, Default)]
pub(super) struct SubmitOptionsBody {
    pub timeout: Option<i64>,
    pub priority: Option<i64>,
    pub platform: Option<String>,
    pub tags: Option<StringList>,
    pub owner: Option<String>,
    pub target_filename: Option<String>,
    pub plugins: Option<StringList>,
    pub excluded_plugins: Option<StringList>,
    pub snapshot_id: Option<String>,
    pub machine_id: Option<i32>,
    pub params: Option<SubmissionParams>,
}

#[derive(Deserialize)]
struct ReferenceBody {
    source: TaskSource,
    #[serde(flatten)]
    options: SubmitOptionsBody,
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
                .map_err(IntoResponse::into_response)?;
            Ok(CreateTaskBody::Upload(fields))
        } else if content_type.starts_with("application/json") {
            let Json(body) = Json::<ReferenceBody>::from_request(req, state)
                .await
                .map_err(IntoResponse::into_response)?;
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

/// Raw submission options, before validation.
pub(super) struct SubmitOptions {
    pub timeout: Option<i64>,
    pub priority: Option<i64>,
    pub platform: Option<String>,
    pub tags: Option<Vec<String>>,
    pub owner: Option<String>,
    pub plugins: Vec<String>,
    pub excluded_plugins: Vec<String>,
    pub snapshot_id: Option<String>,
    pub machine_id: Option<i32>,
    pub params: SubmissionParams,
}

impl From<SubmitOptionsBody> for SubmitOptions {
    fn from(b: SubmitOptionsBody) -> Self {
        SubmitOptions {
            timeout: b.timeout,
            priority: b.priority,
            platform: b.platform,
            tags: b.tags.map(StringList::into_vec),
            owner: b.owner,
            plugins: b.plugins.map(StringList::into_vec).unwrap_or_default(),
            excluded_plugins: b
                .excluded_plugins
                .map(StringList::into_vec)
                .unwrap_or_default(),
            snapshot_id: b.snapshot_id,
            machine_id: b.machine_id,
            params: b.params.unwrap_or_default(),
        }
    }
}

type FieldErrors = HashMap<Cow<'static, str>, Vec<Cow<'static, str>>>;

fn push_error(
    errors: &mut FieldErrors,
    field: impl Into<Cow<'static, str>>,
    msg: impl Into<Cow<'static, str>>,
) {
    errors.entry(field.into()).or_default().push(msg.into());
}

fn parse_platform(s: &str) -> Option<MachinePlatform> {
    match s {
        "linux" => Some(MachinePlatform::Linux),
        "windows" => Some(MachinePlatform::Windows),
        _ => None,
    }
}

/// Validate the options and create the task with its root artifact, then
/// hand it to the orchestrator.
pub(super) async fn submit(
    state: &AppState,
    sample: SampleEntity,
    identification: Identification,
    target: String,
    opts: SubmitOptions,
    parent_task_id: Option<i32>,
) -> Result<(Task, TaskArtifact)> {
    let mut errors = FieldErrors::new();

    let platform = match opts.platform.as_deref() {
        None => None,
        Some(p) => {
            let parsed = parse_platform(p);
            if parsed.is_none() {
                push_error(&mut errors, "platform", "must be windows or linux");
            }
            parsed
        }
    };
    let snapshot_id = opts.snapshot_id.as_deref().and_then(|s| {
        let parsed = uuid::Uuid::parse_str(s).ok();
        if parsed.is_none() {
            push_error(&mut errors, "snapshot_id", "invalid uuid");
        }
        parsed
    });
    if let Some(machine_id) = opts.machine_id
        && fetch_machine(&state.pool, machine_id)
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch machine: {e}")))?
            .is_none()
    {
        push_error(&mut errors, "machine_id", "unknown machine");
    }
    if opts.timeout.is_some_and(|t| t < 1) {
        push_error(&mut errors, "timeout", "must be at least 1");
    }
    if let Err(pairs) = opts
        .params
        .validate(&state.config.orchestration, state.orchestrator.routing())
    {
        for (field, msg) in pairs {
            push_error(&mut errors, field, msg);
        }
    }
    if !errors.is_empty() {
        return Err(Error::UnprocessableEntity { errors });
    }

    let (task, root) = orchestrator_create_task(
        state.orchestrator.core(),
        NewSubmission {
            sample,
            identification,
            target,
            plugins: opts.plugins,
            excluded_plugins: opts.excluded_plugins,
            timeout: opts.timeout.unwrap_or(300),
            priority: opts.priority.unwrap_or(1),
            platform,
            snapshot_id,
            machine_id: opts.machine_id,
            owner: opts.owner,
            tags: opts.tags,
            params: opts.params,
            parent_task_id,
        },
    )
    .await
    .map_err(|e| Error::Internal(format!("Failed to create task: {e}")))?;

    tracing::Span::current().record("task_id", task.id);
    state.orchestrator.submit(task.id);
    info!(task_id = task.id, "Task submitted to orchestrator");
    Ok((task, root))
}

pub(super) fn created_response(task: &Task, root: &TaskArtifact, sample: SampleEntity) -> Response {
    (
        StatusCode::CREATED,
        Json(CreateTaskResponse {
            task_id: task.id,
            root_artifact_id: root.id,
            sample: SampleDto::from(sample),
        }),
    )
        .into_response()
}

/// Identify a blob off the async runtime.
pub(super) async fn identify_blocking(data: Bytes, name: Option<String>) -> Result<Identification> {
    tokio::task::spawn_blocking(move || identify(&data, name.as_deref()))
        .await
        .map_err(|e| Error::Internal(format!("identification task failed: {e}")))
}

async fn create_from_upload(state: &AppState, request: UploadFields) -> Result<Response> {
    let params = match request.params.as_deref() {
        None => SubmissionParams::default(),
        Some(raw) => serde_json::from_str::<SubmissionParams>(raw)
            .map_err(|e| Error::unprocessable_entity([("params", e.to_string())]))?,
    };

    let file_name = request
        .file
        .metadata
        .file_name
        .clone()
        .unwrap_or_else(|| "data.bin".to_string());
    let contents = request.file.contents.clone();
    let name_for_identify = file_name.clone();
    let (sample, identification) = tokio::task::spawn_blocking(move || {
        let identification = identify(&contents, Some(&name_for_identify));
        let sample = Sample {
            file_size: i64::try_from(contents.len()).expect("in-memory buffer length fits in i64"),
            file_type: identification.description.clone(),
            md5: get_md5(&contents),
            crc32: get_crc32(&contents),
            sha1: get_sha1(&contents),
            sha256: get_sha256(&contents),
            sha512: get_sha512(&contents),
            ssdeep: "not-available".to_string(),
        };
        (sample, identification)
    })
    .await
    .map_err(|e| Error::Internal(format!("hashing task failed: {e}")))?;

    state
        .sample_store
        .store(&sample.sha256, &request.file.contents)
        .await
        .map_err(|e| Error::Internal(format!("Failed to store sample file: {e}")))?;

    let sample = insert_sample(&state.pool, sample)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert sample: {e}")))?;

    let target = request.target_filename.clone().unwrap_or(file_name);
    let opts = SubmitOptions {
        timeout: request.timeout,
        priority: request.priority,
        platform: request.platform,
        tags: request.tags.as_deref().map(split_csv),
        owner: request.owner,
        plugins: request
            .plugins
            .as_deref()
            .map(split_csv)
            .unwrap_or_default(),
        excluded_plugins: request
            .excluded_plugins
            .as_deref()
            .map(split_csv)
            .unwrap_or_default(),
        snapshot_id: request.snapshot_id,
        machine_id: request.machine_id,
        params,
    };

    let (task, root) = submit(state, sample.clone(), identification, target, opts, None).await?;
    Ok(created_response(&task, &root, sample))
}

async fn create_from_sample(
    state: &AppState,
    sample_id: i64,
    options: SubmitOptionsBody,
) -> Result<Response> {
    let sample = fetch_sample_by_id(&state.pool, sample_id)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch sample: {e}")))?
        .ok_or(Error::NotFound)?;

    let missing =
        || Error::unprocessable_entity([("sample", "Sample file no longer exists on disk")]);
    if !state
        .sample_store
        .exists(&sample.sha256)
        .await
        .map_err(|e| Error::Internal(format!("Failed to check sample store: {e}")))?
    {
        return Err(missing());
    }
    let path = state
        .sample_store
        .path(&sample.sha256)
        .map_err(|e| Error::Internal(format!("Failed to resolve sample path: {e}")))?;
    let data = tokio::fs::read(&path).await.map_err(|_| missing())?;

    let target = options
        .target_filename
        .clone()
        .unwrap_or_else(|| sample.sha256.clone());
    let identification = identify_blocking(Bytes::from(data), Some(target.clone())).await?;

    let (task, root) = submit(
        state,
        sample.clone(),
        identification,
        target,
        options.into(),
        None,
    )
    .await?;
    Ok(created_response(&task, &root, sample))
}

#[tracing::instrument(skip_all, fields(task_id = tracing::field::Empty))]
async fn create_task(State(state): State<AppState>, body: CreateTaskBody) -> Result<Response> {
    match body {
        CreateTaskBody::Upload(fields) => create_from_upload(&state, fields).await,
        CreateTaskBody::Reference(body) => match body.source {
            TaskSource::SampleId(id) => create_from_sample(&state, id, body.options).await,
            TaskSource::Url(_) | TaskSource::Hash(_) => Err(Error::NotImplemented(
                "url/hash ingestion not implemented".to_string(),
            )),
        },
    }
}
