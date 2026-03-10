use crate::error::{ImageError, Result};
use crate::repositories::machinery::{MachineArch, MachinePlatform};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Image {
    pub id: Uuid,
    pub name: String,
    pub platform: MachinePlatform,
    pub arch: MachineArch,
    pub format: String,
    pub description: Option<String>,
    pub path: String,
    pub available: bool,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

pub struct NewImage {
    pub name: String,
    pub platform: MachinePlatform,
    pub arch: MachineArch,
    pub format: String,
    pub description: Option<String>,
    pub path: String,
}

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
