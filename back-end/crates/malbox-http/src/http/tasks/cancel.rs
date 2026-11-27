use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use tracing::info;

use super::super::AppState;
use super::control_error;
use crate::http::Result;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/tasks/{id}/cancel", post(cancel_task))
}

async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>> {
    state
        .orchestrator
        .cancel_task(id)
        .await
        .map_err(control_error)?;
    info!(task_id = id, "Task cancellation requested");
    Ok(Json(
        serde_json::json!({ "status": "canceled", "task_id": id }),
    ))
}
