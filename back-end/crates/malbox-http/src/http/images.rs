use crate::http::{AppState, Result, error::Error};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use malbox_database::repositories::images::{self, NewImage};
use malbox_database::repositories::machinery::{MachineArch, MachinePlatform};
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/images", post(register_image))
        .route("/v1/images", get(list_images))
        .route("/v1/images/{name}", get(get_image))
        .route("/v1/images/{name}", delete(delete_image))
}

#[derive(Deserialize)]
struct RegisterImageRequest {
    name: String,
    platform: String,
    arch: String,
    format: Option<String>,
    description: Option<String>,
    path: String,
}

async fn register_image(
    State(state): State<AppState>,
    Json(request): Json<RegisterImageRequest>,
) -> Result<impl IntoResponse> {
    let platform = match request.platform.to_lowercase().as_str() {
        "windows" => MachinePlatform::Windows,
        "linux" => MachinePlatform::Linux,
        _ => {
            return Err(Error::unprocessable_entity([(
                "platform",
                "must be 'windows' or 'linux'",
            )]))
        }
    };

    let arch = match request.arch.to_lowercase().as_str() {
        "x64" => MachineArch::X64,
        "x86" => MachineArch::X86,
        _ => {
            return Err(Error::unprocessable_entity([(
                "arch",
                "must be 'x64' or 'x86'",
            )]))
        }
    };

    if !std::path::Path::new(&request.path).exists() {
        return Err(Error::unprocessable_entity([(
            "path",
            "file does not exist at the specified path",
        )]));
    }

    let new_image = NewImage {
        name: request.name,
        platform,
        arch,
        format: request.format.unwrap_or_else(|| "qcow2".to_string()),
        description: request.description,
        path: request.path,
    };

    let image = images::insert_image(&state.pool, new_image)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert image: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": image.id,
            "name": image.name,
            "available": image.available,
        })),
    ))
}

async fn list_images(State(state): State<AppState>) -> Result<impl IntoResponse> {
    let images = images::fetch_all_images(&state.pool)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch images: {}", e)))?;

    Ok(Json(images))
}

async fn get_image(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse> {
    let image = images::fetch_image_by_name(&state.pool, &name)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch image: {}", e)))?;

    match image {
        Some(img) => Ok(Json(img).into_response()),
        None => Ok((StatusCode::NOT_FOUND, "Image not found").into_response()),
    }
}

async fn delete_image(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse> {
    let image = images::delete_image_by_name(&state.pool, &name)
        .await
        .map_err(|e| Error::Internal(format!("Failed to delete image: {}", e)))?;

    match image {
        Some(_) => Ok(StatusCode::NO_CONTENT.into_response()),
        None => Ok((StatusCode::NOT_FOUND, "Image not found").into_response()),
    }
}
