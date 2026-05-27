use crate::http::{AppState, Result, error::Error};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_database::repositories::recipes::{
    RecipeScope, fetch_all_recipes, fetch_recipe, fetch_recipes_by_author, fetch_recipes_by_scope,
    fetch_recipes_by_tag,
};
use serde::Deserialize;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/recipes", get(list_recipes))
        .route("/v1/recipes/{id}", get(get_recipe))
}

#[derive(Deserialize)]
struct ListRecipesQuery {
    author: Option<String>,
    scope: Option<String>,
    tag: Option<String>,
}

async fn list_recipes(
    State(state): State<AppState>,
    Query(query): Query<ListRecipesQuery>,
) -> Result<impl IntoResponse> {
    let recipes = if let Some(author) = query.author {
        fetch_recipes_by_author(&state.pool, &author)
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch recipes: {}", e)))?
    } else if let Some(scope_str) = query.scope {
        let scope = match scope_str.to_lowercase().as_str() {
            "personal" => RecipeScope::Personal,
            "shared" => RecipeScope::Shared,
            _ => {
                return Err(Error::unprocessable_entity([(
                    "scope",
                    "must be 'personal' or 'shared'",
                )]));
            }
        };
        fetch_recipes_by_scope(&state.pool, scope)
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch recipes: {}", e)))?
    } else if let Some(tag) = query.tag {
        fetch_recipes_by_tag(&state.pool, &tag)
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch recipes: {}", e)))?
    } else {
        fetch_all_recipes(&state.pool)
            .await
            .map_err(|e| Error::Internal(format!("Failed to fetch recipes: {}", e)))?
    };

    Ok(Json(recipes))
}

async fn get_recipe(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let recipe = fetch_recipe(&state.pool, id)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch recipe: {}", e)))?;

    match recipe {
        Some(r) => Ok(Json(r).into_response()),
        None => Ok((StatusCode::NOT_FOUND, "Recipe not found").into_response()),
    }
}
