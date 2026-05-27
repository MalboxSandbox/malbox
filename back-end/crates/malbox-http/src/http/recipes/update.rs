use crate::http::{AppState, Result, error::Error};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::put,
};
use malbox_database::repositories::recipes::{RecipeScope, RecipeUpdate, update_recipe};
use serde::Deserialize;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/recipes/{id}", put(update_recipe_handler))
}

#[derive(Deserialize)]
struct UpdateRecipeRequest {
    name: Option<String>,
    description: Option<String>,
    scope: Option<String>,
    tags: Option<Vec<String>>,
    steps: Option<serde_json::Value>,
}

async fn update_recipe_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateRecipeRequest>,
) -> Result<impl IntoResponse> {
    let scope = match request.scope.as_deref().map(str::to_lowercase).as_deref() {
        Some("personal") => Some(RecipeScope::Personal),
        Some("shared") => Some(RecipeScope::Shared),
        Some(_) => {
            return Err(Error::unprocessable_entity([(
                "scope",
                "must be 'personal' or 'shared'",
            )]));
        }
        None => None,
    };

    let update = RecipeUpdate {
        name: request.name,
        description: request.description,
        scope,
        tags: request.tags,
        steps: request.steps,
    };

    let recipe = update_recipe(&state.pool, id, update)
        .await
        .map_err(|e| Error::Internal(format!("Failed to update recipe: {}", e)))?;

    match recipe {
        Some(r) => Ok(Json(r).into_response()),
        None => Ok((StatusCode::NOT_FOUND, "Recipe not found").into_response()),
    }
}
