//! `custom_transforms`: user-authored report transforms, edited in the UI or synced from git.

use crate::error::{CustomTransformError, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use time::PrimitiveDateTime;
use uuid::Uuid;

/// Language a transform is written in (`transform_kind` Postgres enum).
#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[sqlx(type_name = "transform_kind", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "mirrors the `transform_kind` Postgres enum; the variant set is fixed by the schema"
)]
pub enum TransformKind {
    /// Declarative YAML transform.
    Yaml,
    /// JavaScript transform.
    Js,
    /// WebAssembly module transform.
    Wasm,
}

/// A stored transform row.
#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct CustomTransform {
    /// Database primary key.
    pub id: Uuid,
    /// Stable user-facing identifier, unique across transforms.
    pub transform_id: String,
    /// Display name.
    pub name: String,
    /// Grouping category shown in the UI.
    pub category: String,
    /// Language of `content`.
    pub kind: TransformKind,
    /// Transform source code or module payload.
    pub content: String,
    /// Whether the transform is applied to reports.
    pub enabled: bool,
    /// Whether the row is owned by git sync and read-only in the UI.
    pub git_synced: bool,
    /// When the row was inserted.
    pub created_on: PrimitiveDateTime,
    /// When the row was last modified, if ever.
    pub updated_at: Option<PrimitiveDateTime>,
}

/// Column values for inserting a [`CustomTransform`].
#[derive(Debug)]
pub struct NewCustomTransform {
    /// Stable user-facing identifier, unique across transforms.
    pub transform_id: String,
    /// Display name.
    pub name: String,
    /// Grouping category shown in the UI.
    pub category: String,
    /// Language of `content`.
    pub kind: TransformKind,
    /// Transform source code or module payload.
    pub content: String,
    /// Whether the transform is applied to reports.
    pub enabled: bool,
    /// Whether the row is owned by git sync.
    pub git_synced: bool,
}

/// Insert a new transform.
///
/// # Errors
///
/// Returns [`CustomTransformError::InsertFailed`] when the insert fails,
/// e.g. on a duplicate `transform_id`.
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

/// Fetch a transform by primary key, `None` when it does not exist.
///
/// # Errors
///
/// Returns [`CustomTransformError::FetchFailed`] when the query fails.
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

/// Fetch a transform by its user-facing `transform_id`.
///
/// # Errors
///
/// Returns [`CustomTransformError::FetchFailed`] when the query fails.
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

/// Every transform, newest first.
///
/// # Errors
///
/// Returns [`CustomTransformError::FetchFailed`] when the query fails.
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

/// Partial update of a transform; `None` fields keep their stored value.
#[derive(Debug)]
pub struct CustomTransformUpdate {
    /// New display name.
    pub name: Option<String>,
    /// New grouping category.
    pub category: Option<String>,
    /// New language of `content`.
    pub kind: Option<TransformKind>,
    /// New source code or module payload.
    pub content: Option<String>,
    /// New enabled flag.
    pub enabled: Option<bool>,
}

/// Update a non-git-synced custom transform. Returns `None` if the transform
/// does not exist or is git-synced (and therefore not editable via UI).
///
/// # Errors
///
/// Returns [`CustomTransformError::UpdateFailed`] when the update fails.
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
///
/// # Errors
///
/// Returns [`CustomTransformError::DeleteFailed`] when the delete fails.
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
///
/// # Errors
///
/// Returns [`CustomTransformError::InsertFailed`] when the upsert fails.
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
