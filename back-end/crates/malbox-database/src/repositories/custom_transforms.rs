use crate::error::{CustomTransformError, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use time::PrimitiveDateTime;
use uuid::Uuid;

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[sqlx(type_name = "transform_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum TransformKind {
    Yaml,
    Js,
    Wasm,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct CustomTransform {
    pub id: Uuid,
    pub transform_id: String,
    pub name: String,
    pub category: String,
    pub kind: TransformKind,
    pub content: String,
    pub enabled: bool,
    pub git_synced: bool,
    pub created_on: PrimitiveDateTime,
    pub updated_at: Option<PrimitiveDateTime>,
}

pub struct NewCustomTransform {
    pub transform_id: String,
    pub name: String,
    pub category: String,
    pub kind: TransformKind,
    pub content: String,
    pub enabled: bool,
    pub git_synced: bool,
}

pub async fn insert_transform(pool: &PgPool, new: NewCustomTransform) -> Result<CustomTransform> {
    sqlx::query_as::<_, CustomTransform>(
        r#"
        INSERT INTO "custom_transforms" (transform_id, name, category, kind, content, enabled, git_synced)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
    )
    .bind(&new.transform_id)
    .bind(&new.name)
    .bind(&new.category)
    .bind(&new.kind)
    .bind(&new.content)
    .bind(new.enabled)
    .bind(new.git_synced)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        CustomTransformError::InsertFailed {
            name: new.name,
            message: "failed to insert custom transform record".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn fetch_transform(pool: &PgPool, id: Uuid) -> Result<Option<CustomTransform>> {
    sqlx::query_as::<_, CustomTransform>(
        r#"
        SELECT * FROM "custom_transforms" WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| CustomTransformError::FetchFailed { source: e }.into())
}

pub async fn fetch_transform_by_transform_id(
    pool: &PgPool,
    transform_id: &str,
) -> Result<Option<CustomTransform>> {
    sqlx::query_as::<_, CustomTransform>(
        r#"
        SELECT * FROM "custom_transforms" WHERE transform_id = $1
        "#,
    )
    .bind(transform_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| CustomTransformError::FetchFailed { source: e }.into())
}

pub async fn fetch_all_transforms(pool: &PgPool) -> Result<Vec<CustomTransform>> {
    sqlx::query_as::<_, CustomTransform>(
        r#"
        SELECT * FROM "custom_transforms" ORDER BY created_on DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| CustomTransformError::FetchFailed { source: e }.into())
}

pub struct CustomTransformUpdate {
    pub name: Option<String>,
    pub category: Option<String>,
    pub kind: Option<TransformKind>,
    pub content: Option<String>,
    pub enabled: Option<bool>,
}

/// Update a non-git-synced custom transform. Returns `None` if the transform
/// does not exist or is git-synced (and therefore not editable via UI).
pub async fn update_transform(
    pool: &PgPool,
    id: Uuid,
    update: CustomTransformUpdate,
) -> Result<Option<CustomTransform>> {
    sqlx::query_as::<_, CustomTransform>(
        r#"
        UPDATE "custom_transforms"
        SET
            name     = COALESCE($1, name),
            category = COALESCE($2, category),
            kind     = COALESCE($3, kind),
            content  = COALESCE($4, content),
            enabled  = COALESCE($5, enabled)
        WHERE id = $6 AND git_synced = false
        RETURNING *
        "#,
    )
    .bind(update.name)
    .bind(update.category)
    .bind(update.kind)
    .bind(update.content)
    .bind(update.enabled)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        CustomTransformError::UpdateFailed {
            message: "failed to update custom transform record".to_string(),
            source: e,
        }
        .into()
    })
}

/// Delete a non-git-synced custom transform. Returns `None` if the transform
/// does not exist or is git-synced (and therefore not deletable via UI).
pub async fn delete_transform(pool: &PgPool, id: Uuid) -> Result<Option<CustomTransform>> {
    sqlx::query_as::<_, CustomTransform>(
        r#"
        DELETE FROM "custom_transforms" WHERE id = $1 AND git_synced = false RETURNING *
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| CustomTransformError::DeleteFailed { source: e }.into())
}

/// Idempotently insert or update a git-synced custom transform.
/// Used during git sync to apply the authoritative state from the repository.
pub async fn upsert_git_synced(pool: &PgPool, new: NewCustomTransform) -> Result<CustomTransform> {
    sqlx::query_as::<_, CustomTransform>(
        r#"
        INSERT INTO "custom_transforms" (transform_id, name, category, kind, content, enabled, git_synced)
        VALUES ($1, $2, $3, $4, $5, $6, true)
        ON CONFLICT (transform_id) DO UPDATE
        SET
            name       = EXCLUDED.name,
            category   = EXCLUDED.category,
            kind       = EXCLUDED.kind,
            content    = EXCLUDED.content,
            enabled    = EXCLUDED.enabled,
            git_synced = true
        RETURNING *
        "#,
    )
    .bind(&new.transform_id)
    .bind(&new.name)
    .bind(&new.category)
    .bind(&new.kind)
    .bind(&new.content)
    .bind(new.enabled)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        CustomTransformError::InsertFailed {
            name: new.name,
            message: "failed to upsert git-synced transform".to_string(),
            source: e,
        }
        .into()
    })
}
