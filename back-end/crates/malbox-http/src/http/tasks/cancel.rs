use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use malbox_database::repositories::tasks::{TaskState, fetch_task, update_task_status};
use tracing::info;

use super::super::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/tasks/{id}/cancel", post(cancel_task))
}

async fn cancel_task(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    let task = match fetch_task(&state.pool, id).await {
        Ok(Some(t)) => t,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": format!("Task {} not found", id)})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    match task.status {
        TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
            return (
                StatusCode::CONFLICT,
                Json(
                    serde_json::json!({"error": format!("Task {} is already in terminal state: {:?}", id, task.status)}),
                ),
            )
                .into_response();
        }
        TaskState::Pending => {
            // Task is still in the queue - mark as cancelled in DB.
            // The worker will skip it when dequeued.
            if let Err(e) = update_task_status(&state.pool, id, TaskState::Canceled).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
                    .into_response();
            }
            info!(task_id = id, "Pending task cancelled");
        }
        _ => {
            // Task is running/initializing/preparing/stopping - try to cancel via registry
            let found = state.cancel_registry.cancel(id).await;
            if !found {
                // Token not in registry (edge case: task between states).
                // Fall back to direct DB update.
                if let Err(e) = update_task_status(&state.pool, id, TaskState::Canceled).await {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                        .into_response();
                }
            }
            info!(
                task_id = id,
                in_registry = found,
                "Running task cancellation requested"
            );
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "cancelled", "task_id": id})),
    )
        .into_response()
}
