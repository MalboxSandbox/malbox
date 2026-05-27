use crate::http::{AppState, Result, error::Error};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::put,
};
use malbox_database::repositories::custom_transforms::{
    CustomTransformUpdate, TransformKind, update_transform,
};
use serde::Deserialize;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/transforms/{id}", put(update_transform_handler))
}

#[derive(Deserialize)]
struct UpdateTransformRequest {
    name: Option<String>,
    category: Option<String>,
    kind: Option<String>,
    content: Option<String>,
    enabled: Option<bool>,
}

async fn update_transform_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateTransformRequest>,
) -> Result<impl IntoResponse> {
    let kind = match request.kind.as_deref().map(str::to_lowercase).as_deref() {
        Some("yaml") => Some(TransformKind::Yaml),
        Some("js") => Some(TransformKind::Js),
        Some("wasm") => Some(TransformKind::Wasm),
        Some(_) => {
            return Err(Error::unprocessable_entity([(
                "kind",
                "must be 'yaml', 'js', or 'wasm'",
            )]));
        }
        None => None,
    };

    let update = CustomTransformUpdate {
        name: request.name,
        category: request.category,
        kind,
        content: request.content,
        enabled: request.enabled,
    };

    let transform = update_transform(&state.pool, id, update)
        .await
        .map_err(|e| Error::Internal(format!("Failed to update transform: {}", e)))?;

    match transform {
        Some(t) => Ok(Json(t).into_response()),
        None => Ok((
            StatusCode::NOT_FOUND,
            "Transform not found or is git-synced",
        )
            .into_response()),
    }
}
