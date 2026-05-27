use crate::http::{AppState, Result, error::Error};
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::delete,
};
use malbox_database::repositories::custom_transforms::delete_transform;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/transforms/{id}", delete(delete_transform_handler))
}

async fn delete_transform_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let transform = delete_transform(&state.pool, id)
        .await
        .map_err(|e| Error::Internal(format!("Failed to delete transform: {}", e)))?;

    match transform {
        Some(_) => Ok(StatusCode::NO_CONTENT.into_response()),
        None => Ok((
            StatusCode::NOT_FOUND,
            "Transform not found or is git-synced",
        )
            .into_response()),
    }
}
