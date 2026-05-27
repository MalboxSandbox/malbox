use crate::http::{AppState, Result, error::Error};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_database::repositories::custom_transforms::{fetch_all_transforms, fetch_transform};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/transforms", get(list_transforms))
        .route("/v1/transforms/{id}", get(get_transform))
}

async fn list_transforms(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let transforms = fetch_all_transforms(&state.pool)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch transforms: {}", e)))?;

    Ok(Json(transforms))
}

async fn get_transform(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let transform = fetch_transform(&state.pool, id)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch transform: {}", e)))?;

    match transform {
        Some(t) => Ok(Json(t).into_response()),
        None => Ok((StatusCode::NOT_FOUND, "Transform not found").into_response()),
    }
}
