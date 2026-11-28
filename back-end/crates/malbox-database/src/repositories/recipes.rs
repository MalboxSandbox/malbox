//! `recipes`: reusable provisioning step lists, personal or shared.

use crate::error::{RecipeError, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use time::PrimitiveDateTime;
use uuid::Uuid;

/// Visibility of a recipe (`recipe_scope` Postgres enum).
#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[sqlx(type_name = "recipe_scope", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "mirrors the `recipe_scope` Postgres enum; the variant set is fixed by the schema"
)]
pub enum RecipeScope {
    /// Visible only to its author.
    Personal,
    /// Visible to every user.
    Shared,
}

/// A row of the `recipes` table.
#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Recipe {
    /// Database primary key.
    pub id: Uuid,
    /// Display name.
    pub name: String,
    /// Free-form description.
    pub description: Option<String>,
    /// User who created the recipe.
    pub author: String,
    /// Who can see the recipe.
    pub scope: RecipeScope,
    /// Search tags.
    pub tags: Vec<String>,
    /// Provisioning steps as a JSON array.
    pub steps: serde_json::Value,
    /// When the row was inserted.
    pub created_on: PrimitiveDateTime,
    /// When the row was last modified, if ever.
    pub updated_at: Option<PrimitiveDateTime>,
}

/// Column values for inserting a [`Recipe`].
#[derive(Debug)]
pub struct NewRecipe {
    /// Display name.
    pub name: String,
    /// Free-form description.
    pub description: Option<String>,
    /// User creating the recipe.
    pub author: String,
    /// Who can see the recipe.
    pub scope: RecipeScope,
    /// Search tags.
    pub tags: Vec<String>,
    /// Provisioning steps as a JSON array.
    pub steps: serde_json::Value,
}

/// Insert a new recipe.
///
/// # Errors
///
/// Returns [`RecipeError::InsertFailed`] when the insert fails.
pub async fn insert_recipe(pool: &PgPool, new_recipe: NewRecipe) -> Result<Recipe> {
    sqlx::query_as::<_, Recipe>(
        r#"
        INSERT INTO "recipes" (name, description, author, scope, tags, steps)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING *
        "#,
    )
    .bind(&new_recipe.name)
    .bind(&new_recipe.description)
    .bind(&new_recipe.author)
    .bind(&new_recipe.scope)
    .bind(&new_recipe.tags)
    .bind(&new_recipe.steps)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        RecipeError::InsertFailed {
            name: new_recipe.name,
            message: "failed to insert recipe record".to_string(),
            source: e,
        }
        .into()
    })
}

/// Fetch a recipe by primary key, `None` when it does not exist.
///
/// # Errors
///
/// Returns [`RecipeError::FetchFailed`] when the query fails.
pub async fn fetch_recipe(pool: &PgPool, id: Uuid) -> Result<Option<Recipe>> {
    sqlx::query_as::<_, Recipe>(
        r#"
        SELECT * FROM "recipes" WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| RecipeError::FetchFailed { source: e }.into())
}

/// Every recipe, newest first.
///
/// # Errors
///
/// Returns [`RecipeError::FetchFailed`] when the query fails.
pub async fn fetch_all_recipes(pool: &PgPool) -> Result<Vec<Recipe>> {
    sqlx::query_as::<_, Recipe>(
        r#"
        SELECT * FROM "recipes" ORDER BY created_on DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| RecipeError::FetchFailed { source: e }.into())
}

/// Recipes created by `author`, newest first.
///
/// # Errors
///
/// Returns [`RecipeError::FetchFailed`] when the query fails.
pub async fn fetch_recipes_by_author(pool: &PgPool, author: &str) -> Result<Vec<Recipe>> {
    sqlx::query_as::<_, Recipe>(
        r#"
        SELECT * FROM "recipes" WHERE author = $1 ORDER BY created_on DESC
        "#,
    )
    .bind(author)
    .fetch_all(pool)
    .await
    .map_err(|e| RecipeError::FetchFailed { source: e }.into())
}

/// Recipes with the given visibility, newest first.
///
/// # Errors
///
/// Returns [`RecipeError::FetchFailed`] when the query fails.
pub async fn fetch_recipes_by_scope(pool: &PgPool, scope: RecipeScope) -> Result<Vec<Recipe>> {
    sqlx::query_as::<_, Recipe>(
        r#"
        SELECT * FROM "recipes" WHERE scope = $1 ORDER BY created_on DESC
        "#,
    )
    .bind(scope)
    .fetch_all(pool)
    .await
    .map_err(|e| RecipeError::FetchFailed { source: e }.into())
}

/// Recipes carrying `tag`, newest first.
///
/// # Errors
///
/// Returns [`RecipeError::FetchFailed`] when the query fails.
pub async fn fetch_recipes_by_tag(pool: &PgPool, tag: &str) -> Result<Vec<Recipe>> {
    sqlx::query_as::<_, Recipe>(
        r#"
        SELECT * FROM "recipes" WHERE $1 = ANY(tags) ORDER BY created_on DESC
        "#,
    )
    .bind(tag)
    .fetch_all(pool)
    .await
    .map_err(|e| RecipeError::FetchFailed { source: e }.into())
}

/// Partial update of a recipe; `None` fields keep their stored value.
#[derive(Debug)]
pub struct RecipeUpdate {
    /// New display name.
    pub name: Option<String>,
    /// New description.
    pub description: Option<String>,
    /// New visibility.
    pub scope: Option<RecipeScope>,
    /// New search tags.
    pub tags: Option<Vec<String>>,
    /// New provisioning steps.
    pub steps: Option<serde_json::Value>,
}

/// Apply `update` to a recipe, returning the new row or `None` if it does not exist.
///
/// # Errors
///
/// Returns [`RecipeError::UpdateFailed`] when the update fails.
pub async fn update_recipe(
    pool: &PgPool,
    id: Uuid,
    update: RecipeUpdate,
) -> Result<Option<Recipe>> {
    sqlx::query_as::<_, Recipe>(
        r#"
        UPDATE "recipes"
        SET
            name        = COALESCE($1, name),
            description = COALESCE($2, description),
            scope       = COALESCE($3, scope),
            tags        = COALESCE($4, tags),
            steps       = COALESCE($5, steps)
        WHERE id = $6
        RETURNING *
        "#,
    )
    .bind(update.name)
    .bind(update.description)
    .bind(update.scope)
    .bind(update.tags)
    .bind(update.steps)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        RecipeError::UpdateFailed {
            message: "failed to update recipe record".to_string(),
            source: e,
        }
        .into()
    })
}

/// Delete a recipe, returning the removed row or `None` if it did not exist.
///
/// # Errors
///
/// Returns [`RecipeError::DeleteFailed`] when the delete fails.
pub async fn delete_recipe(pool: &PgPool, id: Uuid) -> Result<Option<Recipe>> {
    sqlx::query_as::<_, Recipe>(
        r#"
        DELETE FROM "recipes" WHERE id = $1 RETURNING *
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| RecipeError::DeleteFailed { source: e }.into())
}
