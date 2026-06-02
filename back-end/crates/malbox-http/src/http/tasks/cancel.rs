use axum::{
    Json, Router,
    extract::{Path, State},
    routing::post,
};
use malbox_database::repositories::tasks::{TaskState, fetch_task, update_task_status};
use tracing::info;

use super::super::AppState;
use crate::http::{Result, error::Error};

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/tasks/{id}/cancel", post(cancel_task))
}

async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<serde_json::Value>> {
    let task = fetch_task(&state.pool, id)
        .await
        .map_err(|e| Error::Internal(e.to_string()))?
        .ok_or(Error::NotFound)?;

    match task.status {
        TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
            return Err(Error::Conflict(format!(
                "Task {} is already in terminal state: {:?}",
                id, task.status
            )));
        }
        TaskState::Pending => {
            update_task_status(&state.pool, id, TaskState::Canceled)
                .await
                .map_err(|e| Error::Internal(e.to_string()))?;
            info!(task_id = id, "Pending task cancelled");
        }
        _ => {
            let found = state.cancel_registry.cancel(id).await;
            if !found {
                update_task_status(&state.pool, id, TaskState::Canceled)
                    .await
                    .map_err(|e| Error::Internal(e.to_string()))?;
            }
            info!(
                task_id = id,
                in_registry = found,
                "Running task cancellation requested"
            );
        }
    }
    Ok(Json(
        serde_json::json!({ "status": "cancelled", "task_id": id }),
    ))
}
