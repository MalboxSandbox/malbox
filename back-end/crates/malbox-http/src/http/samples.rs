use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_database::repositories::{
    samples::fetch_sample_by_hash, tasks::fetch_tasks_by_sample_id,
};
use serde::{Deserialize, Serialize};

use super::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/samples/lookup", get(lookup_sample))
}

#[derive(Deserialize)]
struct LookupParams {
    md5: Option<String>,
    sha1: Option<String>,
    sha256: Option<String>,
    sha512: Option<String>,
}

#[derive(Serialize)]
struct SampleLookupResponse {
    sample: SampleInfo,
    task_ids: Vec<i32>,
}

#[derive(Serialize)]
struct SampleInfo {
    id: i64,
    file_size: i64,
    file_type: String,
    md5: String,
    crc32: String,
    sha1: String,
    sha256: String,
    sha512: String,
    ssdeep: String,
}

async fn lookup_sample(
    State(state): State<AppState>,
    Query(params): Query<LookupParams>,
) -> impl IntoResponse {
    let (hash_type, hash_value) = if let Some(v) = &params.sha256 {
        ("sha256", v.as_str())
    } else if let Some(v) = &params.sha1 {
        ("sha1", v.as_str())
    } else if let Some(v) = &params.md5 {
        ("md5", v.as_str())
    } else if let Some(v) = &params.sha512 {
        ("sha512", v.as_str())
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "provide one of: sha256, sha1, md5, sha512"})),
        )
            .into_response();
    };

    let sample = match fetch_sample_by_hash(&state.pool, hash_type, hash_value).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "sample not found"})),
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

    let task_ids = match fetch_tasks_by_sample_id(&state.pool, sample.id).await {
        Ok(ids) => ids,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let response = SampleLookupResponse {
        sample: SampleInfo {
            id: sample.id,
            file_size: sample.file_size,
            file_type: sample.file_type,
            md5: sample.md5,
            crc32: sample.crc32,
            sha1: sample.sha1,
            sha256: sample.sha256,
            sha512: sample.sha512,
            ssdeep: sample.ssdeep,
        },
        task_ids,
    };

    (StatusCode::OK, Json(serde_json::json!(response))).into_response()
}
