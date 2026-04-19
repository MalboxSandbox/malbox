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
use tracing::info;

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
            )]));
        }
    };

    let arch = match request.arch.to_lowercase().as_str() {
        "x64" => MachineArch::X64,
        "x86" => MachineArch::X86,
        _ => {
            return Err(Error::unprocessable_entity([(
                "arch",
                "must be 'x64' or 'x86'",
            )]));
        }
    };

    let src = std::path::Path::new(&request.path)
        .canonicalize()
        .map_err(|e| {
            Error::unprocessable_entity([("path", format!("cannot resolve path: {}", e))])
        })?;

    if !src.exists() {
        return Err(Error::unprocessable_entity([(
            "path",
            "file does not exist at the specified path".to_string(),
        )]));
    }

    // Move image to managed store directory
    let format = request.format.unwrap_or_else(|| "qcow2".to_string());
    let images_dir = state
        .config
        .images
        .as_ref()
        .map(|c| std::path::PathBuf::from(&c.store_path))
        .unwrap_or_else(|| std::path::PathBuf::from("/var/lib/malbox/images"));
    tokio::fs::create_dir_all(&images_dir)
        .await
        .map_err(|e| Error::Internal(format!("Failed to create images directory: {}", e)))?;

    let filename = format!("{}.{}", &request.name, &format);
    let dest = images_dir.join(&filename);

    info!(
        src = %src.display(),
        dest = %dest.display(),
        "Moving image to managed store"
    );

    if let Err(_) = tokio::fs::rename(&src, &dest).await {
        // rename fails across filesystems — fall back to copy + remove
        tokio::fs::copy(&src, &dest)
            .await
            .map_err(|e| Error::Internal(format!("Failed to copy image to store: {}", e)))?;
        tokio::fs::remove_file(&src)
            .await
            .map_err(|e| Error::Internal(format!("Failed to remove source image: {}", e)))?;
    }

    let dest_canonical = dest
        .canonicalize()
        .map_err(|e| Error::Internal(format!("Failed to canonicalize destination path: {}", e)))?;

    let dest_str = dest_canonical
        .to_str()
        .ok_or_else(|| Error::Internal("Image destination path is not valid UTF-8".to_string()))?
        .to_string();

    let new_image = NewImage {
        name: request.name,
        platform,
        arch,
        format,
        description: request.description,
        path: dest_str,
    };

    let image = images::insert_image(&state.pool, new_image)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert image: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": image.id,
            "name": image.name,
            "path": image.path,
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
