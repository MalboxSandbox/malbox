use crate::http::AppState;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_plugin_internal::registry::manifest::PluginTypeConfig;
use serde::{Deserialize, Serialize};

pub fn router() -> Router<AppState> {
    Router::new().route("/v1/plugins", get(list_plugins))
}

#[derive(Deserialize)]
struct PluginQuery {
    #[serde(rename = "type")]
    plugin_type: Option<String>,
}

#[derive(Serialize)]
struct PluginInfo {
    name: String,
    version: String,
    description: Option<String>,
    plugin_type: String,
    state: String,
    execution: String,
    binary_path: String,
    plugin_dir: String,
    status: String,
}

async fn list_plugins(
    State(state): State<AppState>,
    Query(query): Query<PluginQuery>,
) -> impl IntoResponse {
    let snapshot = state.plugin_registry.snapshot();

    let type_filter = query.plugin_type.as_deref().and_then(|t| match t {
        "guest" => Some(PluginTypeConfig::Guest),
        "host" => Some(PluginTypeConfig::Host),
        _ => None,
    });

    let entries: Vec<PluginInfo> = snapshot
        .list()
        .filter(|e| {
            type_filter
                .as_ref()
                .is_none_or(|t| e.manifest.plugin.plugin_type == *t)
        })
        .map(|e| PluginInfo {
            name: e.manifest.plugin.name.clone(),
            version: e.manifest.plugin.version.clone(),
            description: e.manifest.plugin.description.clone(),
            plugin_type: format!("{:?}", e.manifest.plugin.plugin_type).to_lowercase(),
            state: format!("{:?}", e.manifest.runtime.state).to_lowercase(),
            execution: format!("{:?}", e.manifest.runtime.execution).to_lowercase(),
            binary_path: e.binary_path.to_string_lossy().to_string(),
            plugin_dir: e.plugin_dir.to_string_lossy().to_string(),
            status: e.status.to_string(),
        })
        .collect();

    (StatusCode::OK, Json(entries))
}
