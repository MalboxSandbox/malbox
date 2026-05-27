use crate::http::{AppState, Result, error::Error};
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::delete,
};
use malbox_database::repositories::recipes::delete_recipe;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/recipes/{id}", delete(delete_recipe_handler))
}

async fn delete_recipe_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    let recipe = delete_recipe(&state.pool, id)
        .await
        .map_err(|e| Error::Internal(format!("Failed to delete recipe: {}", e)))?;

    match recipe {
        Some(_) => Ok(StatusCode::NO_CONTENT.into_response()),
        None => Ok((StatusCode::NOT_FOUND, "Recipe not found").into_response()),
    }
}
