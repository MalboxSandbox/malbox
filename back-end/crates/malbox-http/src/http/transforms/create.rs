use crate::http::{AppState, Result, error::Error};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use malbox_database::repositories::custom_transforms::{
    NewCustomTransform, TransformKind, fetch_transform_by_transform_id, insert_transform,
};
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/transforms", post(create_transform))
}

#[derive(Deserialize)]
struct CreateTransformRequest {
    transform_id: String,
    name: String,
    category: String,
    kind: String,
    content: String,
    enabled: Option<bool>,
}

async fn create_transform(
    State(state): State<AppState>,
    Json(request): Json<CreateTransformRequest>,
) -> Result<impl IntoResponse> {
    if request.transform_id.trim().is_empty() {
        return Err(Error::unprocessable_entity([(
            "transform_id",
            "must not be empty",
        )]));
    }

    if request.name.trim().is_empty() {
        return Err(Error::unprocessable_entity([("name", "must not be empty")]));
    }

    if request.category.trim().is_empty() {
        return Err(Error::unprocessable_entity([(
            "category",
            "must not be empty",
        )]));
    }

    let kind = match request.kind.to_lowercase().as_str() {
        "yaml" => TransformKind::Yaml,
        "js" => TransformKind::Js,
        "wasm" => TransformKind::Wasm,
        _ => {
            return Err(Error::unprocessable_entity([(
                "kind",
                "must be 'yaml', 'js', or 'wasm'",
            )]));
        }
    };

    // Check for duplicate transform_id
    let existing = fetch_transform_by_transform_id(&state.pool, &request.transform_id)
        .await
        .map_err(|e| Error::Internal(format!("Failed to check for existing transform: {}", e)))?;

    if existing.is_some() {
        return Err(Error::unprocessable_entity([(
            "transform_id",
            "a transform with this ID already exists",
        )]));
    }

    let new_transform = NewCustomTransform {
        transform_id: request.transform_id,
        name: request.name,
        category: request.category,
        kind,
        content: request.content,
        enabled: request.enabled.unwrap_or(true),
        git_synced: false,
    };

    let transform = insert_transform(&state.pool, new_transform)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert transform: {}", e)))?;

    Ok((StatusCode::CREATED, Json(transform)))
}
