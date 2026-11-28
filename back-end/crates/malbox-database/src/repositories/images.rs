//! `images`: registered VM disk images that machines are provisioned from.

use crate::error::{ImageError, Result};
use crate::repositories::machinery::{MachineArch, MachinePlatform};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

/// A registered VM disk image.
#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Image {
    /// Database primary key.
    pub id: Uuid,
    /// Unique image name.
    pub name: String,
    /// Guest operating system of the image.
    pub platform: MachinePlatform,
    /// Guest CPU architecture of the image.
    pub arch: MachineArch,
    /// Disk image format, e.g. `qcow2`.
    pub format: String,
    /// Free-form operator description.
    pub description: Option<String>,
    /// Filesystem path of the image file.
    pub path: String,
    /// Whether the image file was present at the last check.
    pub available: bool,
    /// When the row was inserted.
    pub created_at: OffsetDateTime,
    /// When the row was last modified.
    pub updated_at: OffsetDateTime,
}

/// Column values for registering an [`Image`].
#[derive(Debug)]
pub struct NewImage {
    /// Unique image name.
    pub name: String,
    /// Guest operating system of the image.
    pub platform: MachinePlatform,
    /// Guest CPU architecture of the image.
    pub arch: MachineArch,
    /// Disk image format, e.g. `qcow2`.
    pub format: String,
    /// Free-form operator description.
    pub description: Option<String>,
    /// Filesystem path of the image file.
    pub path: String,
}

/// Register a new image.
///
/// # Errors
///
/// Returns [`ImageError::InsertFailed`] when the insert fails, e.g. on a
/// duplicate name.
pub async fn insert_image(pool: &PgPool, new_image: NewImage) -> Result<Image> {
    sqlx::query_as::<_, Image>(
        r#"
        INSERT INTO "images" (name, platform, arch, format, description, path)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(&new_image.name)
    .bind(&new_image.platform)
    .bind(&new_image.arch)
    .bind(&new_image.format)
    .bind(&new_image.description)
    .bind(&new_image.path)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        ImageError::InsertFailed {
            name: new_image.name,
            message: "failed to insert image record".to_string(),
            source: e,
        }
        .into()
    })
}

/// Fetch an image by name, `None` when it does not exist.
///
/// # Errors
///
/// Returns [`ImageError::FetchFailed`] when the query fails.
pub async fn fetch_image_by_name(pool: &PgPool, name: &str) -> Result<Option<Image>> {
    sqlx::query_as::<_, Image>(
        r#"
        SELECT * FROM "images" WHERE name = $1
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|e| ImageError::FetchFailed { source: e }.into())
}

/// Fetch an image by primary key, `None` when it does not exist.
///
/// # Errors
///
/// Returns [`ImageError::FetchFailed`] when the query fails.
pub async fn fetch_image_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Image>> {
    sqlx::query_as::<_, Image>(
        r#"
        SELECT * FROM "images" WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| ImageError::FetchFailed { source: e }.into())
}

/// Every registered image, ordered by name.
///
/// # Errors
///
/// Returns [`ImageError::FetchFailed`] when the query fails.
pub async fn fetch_all_images(pool: &PgPool) -> Result<Vec<Image>> {
    sqlx::query_as::<_, Image>(
        r#"
        SELECT * FROM "images" ORDER BY name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ImageError::FetchFailed { source: e }.into())
}

/// Record whether the named image's file is present; `None` when no image has that name.
///
/// # Errors
///
/// Returns [`ImageError::UpdateFailed`] when the update fails.
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "`available` is the value written to the boolean column, not a mode switch"
)]
pub async fn update_image_availability(
    pool: &PgPool,
    name: &str,
    available: bool,
) -> Result<Option<Image>> {
    sqlx::query_as::<_, Image>(
        r#"
        UPDATE "images"
        SET available = $1, updated_at = NOW()
        WHERE name = $2
        RETURNING *
        "#,
    )
    .bind(available)
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        ImageError::UpdateFailed {
            message: "failed to update image availability".to_string(),
            source: e,
        }
        .into()
    })
}

/// Delete the named image, returning the removed row or `None` if absent.
///
/// # Errors
///
/// Returns [`ImageError::DeleteFailed`] when the delete fails.
pub async fn delete_image_by_name(pool: &PgPool, name: &str) -> Result<Option<Image>> {
    sqlx::query_as::<_, Image>(
        r#"
        DELETE FROM "images" WHERE name = $1 RETURNING *
        "#,
    )
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        ImageError::DeleteFailed {
            name: name.to_string(),
            source: e,
        }
        .into()
    })
}
