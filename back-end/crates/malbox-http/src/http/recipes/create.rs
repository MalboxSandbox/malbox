use crate::http::{AppState, Result, error::Error};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use malbox_database::repositories::recipes::{NewRecipe, RecipeScope, insert_recipe};
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/recipes", post(create_recipe))
}

#[derive(Deserialize)]
struct CreateRecipeRequest {
    name: String,
    description: Option<String>,
    author: String,
    scope: String,
    tags: Option<Vec<String>>,
    steps: serde_json::Value,
}

async fn create_recipe(
    State(state): State<AppState>,
    Json(request): Json<CreateRecipeRequest>,
) -> Result<impl IntoResponse> {
    if request.name.trim().is_empty() {
        return Err(Error::unprocessable_entity([("name", "must not be empty")]));
    }

    if request.author.trim().is_empty() {
        return Err(Error::unprocessable_entity([(
            "author",
            "must not be empty",
        )]));
    }

    let scope = match request.scope.to_lowercase().as_str() {
        "personal" => RecipeScope::Personal,
        "shared" => RecipeScope::Shared,
        _ => {
            return Err(Error::unprocessable_entity([(
                "scope",
                "must be 'personal' or 'shared'",
            )]));
        }
    };

    let new_recipe = NewRecipe {
        name: request.name,
        description: request.description,
        author: request.author,
        scope,
        tags: request.tags.unwrap_or_default(),
        steps: request.steps,
    };

    let recipe = insert_recipe(&state.pool, new_recipe)
        .await
        .map_err(|e| Error::Internal(format!("Failed to insert recipe: {}", e)))?;

    Ok((StatusCode::CREATED, Json(recipe)))
}
