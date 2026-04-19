use crate::http::AppState;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use malbox_config::provisioning::ProvisionStep;
use malbox_database::repositories::{provision_runs, snapshots};
use malbox_resources::error::ResourceError;
use serde::Deserialize;
use tracing::{error, info};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/machines", get(list_machines))
        .route("/v1/machines/{id}", get(get_machine))
        .route("/v1/machines/{id}/snapshots", get(list_snapshots))
        .route(
            "/v1/machines/{id}/snapshots/{snapshot_id}",
            delete(delete_snapshot),
        )
        .route("/v1/machines/{id}/provision", post(provision_machine))
        .route("/v1/machines/{id}/provisions", get(list_provisions))
}

async fn list_machines(State(state): State<AppState>) -> impl IntoResponse {
    match state.machine_pool.list().await {
        Ok(machines) => (StatusCode::OK, Json(serde_json::json!(machines))).into_response(),
        Err(e) => resource_error_response(e),
    }
}

async fn get_machine(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
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

async fn list_snapshots(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    match snapshots::fetch_snapshots_for_machine(&state.pool, id).await {
        Ok(snaps) => (StatusCode::OK, Json(serde_json::json!(snaps))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn delete_snapshot(
    State(state): State<AppState>,
    Path((machine_id, snapshot_name)): Path<(i32, String)>,
) -> impl IntoResponse {
    // Look up snapshot by name
    let snapshot =
        match snapshots::fetch_snapshot_by_name(&state.pool, machine_id, &snapshot_name).await {
            Ok(Some(s)) => s,
            Ok(None) => {
                return (
                StatusCode::NOT_FOUND,
                Json(
                    serde_json::json!({"error": format!("Snapshot '{}' not found", snapshot_name)}),
                ),
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

    // Delete provision runs that reference this snapshot first
    if let Err(e) =
        provision_runs::delete_provision_runs_for_snapshot(&state.pool, snapshot.id).await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to delete provision runs: {}", e)})),
        )
            .into_response();
    }

    match snapshots::delete_snapshot(&state.pool, snapshot.id).await {
        Ok(Some(_)) => StatusCode::NO_CONTENT.into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Snapshot not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Provisioner-agnostic provision request.
#[derive(Deserialize)]
struct ProvisionMachineRequest {
    /// Provisioner type (e.g., "ansible", "native")
    provisioner: String,
    /// Opaque provisioner config — passed through as-is.
    #[serde(default)]
    config: Option<serde_json::Value>,
    /// Plugin names to include — resolved from the daemon's plugin registry.
    /// Resolved paths are injected into the config under a `plugins` table.
    #[serde(default)]
    plugins: Option<Vec<String>>,
    /// Snapshot name to create after provisioning
    #[serde(default)]
    snapshot: Option<String>,
    /// Snapshot to revert to before provisioning (defaults to "base")
    #[serde(default)]
    revert_to: Option<String>,
}

/// Run a provisioning step against an existing machine.
async fn provision_machine(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<ProvisionMachineRequest>,
) -> impl IntoResponse {
    // Convert the opaque JSON config to a TOML value for the provisioner
    let mut config = match req.config {
        Some(json_val) => json_to_toml(json_val),
        None => toml::Value::Table(toml::map::Map::new()),
    };

    // Resolve plugins from registry and inject into config
    if let Some(ref plugin_names) = req.plugins {
        let registry_snapshot = state.plugin_registry.snapshot();
        let guest_plugins = registry_snapshot
            .by_type(malbox_plugin_internal::registry::manifest::PluginTypeConfig::Guest);

        let mut plugins_table = toml::map::Map::new();
        let base_port: u16 = 50051;

        for (i, name) in plugin_names.iter().enumerate() {
            let found = guest_plugins
                .iter()
                .find(|p| p.manifest.plugin.name == *name);

            match found {
                Some(entry) => {
                    let port = base_port + i as u16;
                    let mut plugin_info = toml::map::Map::new();
                    plugin_info.insert(
                        "binary".to_string(),
                        toml::Value::String(entry.binary_path.to_string_lossy().to_string()),
                    );
                    plugin_info.insert(
                        "manifest".to_string(),
                        toml::Value::String(
                            entry
                                .plugin_dir
                                .join("plugin.toml")
                                .to_string_lossy()
                                .to_string(),
                        ),
                    );
                    plugin_info.insert("port".to_string(), toml::Value::Integer(port as i64));
                    plugins_table.insert(name.clone(), toml::Value::Table(plugin_info));

                    info!(
                        plugin = name.as_str(),
                        port,
                        binary = %entry.binary_path.display(),
                        "Resolved guest plugin for provisioning"
                    );
                }
                None => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": format!("Guest plugin '{}' not found in registry", name)
                        })),
                    )
                        .into_response();
                }
            }
        }

        if !plugins_table.is_empty() {
            // Inject resolved plugins into the config table
            if let toml::Value::Table(ref mut table) = config {
                table.insert("plugins".to_string(), toml::Value::Table(plugins_table));
            }
        }
    }

    let step = ProvisionStep {
        provisioner_type: req.provisioner,
        snapshot: req.snapshot,
        config,
        guest_plugins: req.plugins.unwrap_or_default(),
    };

    match state
        .machine_pool
        .run_provision_step(id, &step, req.revert_to.as_deref())
        .await
    {
        Ok(run) => (StatusCode::OK, Json(serde_json::json!(run))).into_response(),
        Err(e) => resource_error_response(e),
    }
}

async fn list_provisions(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    match malbox_database::repositories::provision_runs::fetch_provision_runs_for_machine(
        &state.pool,
        id,
    )
    .await
    {
        Ok(runs) => (StatusCode::OK, Json(serde_json::json!(runs))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Convert a serde_json::Value to a toml::Value.
fn json_to_toml(val: serde_json::Value) -> toml::Value {
    match val {
        serde_json::Value::Null => toml::Value::String(String::new()),
        serde_json::Value::Bool(b) => toml::Value::Boolean(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => toml::Value::String(s),
        serde_json::Value::Array(arr) => {
            toml::Value::Array(arr.into_iter().map(json_to_toml).collect())
        }
        serde_json::Value::Object(obj) => {
            let mut map = toml::map::Map::new();
            for (k, v) in obj {
                map.insert(k, json_to_toml(v));
            }
            toml::Value::Table(map)
        }
    }
}

/// Map a `ResourceError` to an appropriate HTTP response.
fn resource_error_response(err: ResourceError) -> axum::response::Response {
    error!(error = %err, "Request failed");
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
