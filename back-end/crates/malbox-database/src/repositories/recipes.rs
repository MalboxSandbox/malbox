use crate::error::{RecipeError, Result};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use time::PrimitiveDateTime;
use uuid::Uuid;

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[sqlx(type_name = "recipe_scope", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum RecipeScope {
    Personal,
    Shared,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
pub struct Recipe {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub author: String,
    pub scope: RecipeScope,
    pub tags: Vec<String>,
    pub steps: serde_json::Value,
    pub created_on: PrimitiveDateTime,
    pub updated_at: Option<PrimitiveDateTime>,
}

pub struct NewRecipe {
    pub name: String,
    pub description: Option<String>,
    pub author: String,
    pub scope: RecipeScope,
    pub tags: Vec<String>,
    pub steps: serde_json::Value,
}

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

pub struct RecipeUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub scope: Option<RecipeScope>,
    pub tags: Option<Vec<String>>,
    pub steps: Option<serde_json::Value>,
}

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
