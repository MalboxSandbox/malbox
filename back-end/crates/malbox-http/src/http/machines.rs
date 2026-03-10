use crate::http::AppState;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use malbox_database::repositories::machinery::{MachineArch, MachinePlatform};
use malbox_resources::CreateMachineRequest;
use malbox_resources::error::ResourceError;
use serde::Deserialize;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/machines", post(create_machine))
        .route("/v1/machines", get(list_machines))
        .route("/v1/machines/{id}", get(get_machine))
        .route("/v1/machines/{id}", delete(delete_machine))
        .route("/v1/machines/{id}/retry", post(retry_machine))
}

fn default_arch() -> String {
    "x64".to_string()
}

#[derive(Deserialize)]
struct CreateMachineBody {
    name: String,
    image: String,
    platform: String,
    #[serde(default = "default_arch")]
    arch: String,
    cpus: Option<u32>,
    memory_mb: Option<u64>,
}

async fn create_machine(
    State(state): State<AppState>,
    Json(body): Json<CreateMachineBody>,
) -> impl IntoResponse {
    let platform = match body.platform.to_lowercase().as_str() {
        "windows" => MachinePlatform::Windows,
        "linux" => MachinePlatform::Linux,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "platform must be 'windows' or 'linux'"})),
            )
                .into_response();
        }
    };

    let arch = match body.arch.to_lowercase().as_str() {
        "x64" => MachineArch::X64,
        "x86" => MachineArch::X86,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "arch must be 'x64' or 'x86'"})),
            )
                .into_response();
        }
    };

    let request = CreateMachineRequest {
        name: body.name,
        image: body.image,
        platform,
        arch,
        cpus: body.cpus,
        memory_mb: body.memory_mb,
    };

    match state.machine_pool.create_machine(request).await {
        Ok(machine) => (StatusCode::CREATED, Json(serde_json::json!(machine))).into_response(),
        Err(e) => resource_error_response(e),
    }
}

async fn list_machines(State(state): State<AppState>) -> impl IntoResponse {
    match state.machine_pool.list().await {
        Ok(machines) => (StatusCode::OK, Json(serde_json::json!(machines))).into_response(),
        Err(e) => resource_error_response(e),
    }
}

async fn get_machine(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match state.machine_pool.get(id).await {
        Ok(Some(machine)) => (StatusCode::OK, Json(serde_json::json!(machine))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Machine not found"})),
        )
            .into_response(),
        Err(e) => resource_error_response(e),
    }
}

async fn delete_machine(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match state.machine_pool.delete_machine(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => resource_error_response(e),
    }
}

async fn retry_machine(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    match state.machine_pool.retry(id).await {
        Ok(machine) => (StatusCode::OK, Json(serde_json::json!(machine))).into_response(),
        Err(e) => resource_error_response(e),
    }
}

/// Map a `ResourceError` to an appropriate HTTP response.
fn resource_error_response(err: ResourceError) -> axum::response::Response {
    let (status, message) = match &err {
        ResourceError::MachineNotFound { .. } => (StatusCode::NOT_FOUND, err.to_string()),
        ResourceError::MachineAssigned { .. } => (StatusCode::CONFLICT, err.to_string()),
        ResourceError::InvalidMachineState { .. } => (StatusCode::CONFLICT, err.to_string()),
        ResourceError::Database(msg) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Database error: {}", msg),
        ),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };

    (status, Json(serde_json::json!({"error": message}))).into_response()
}
