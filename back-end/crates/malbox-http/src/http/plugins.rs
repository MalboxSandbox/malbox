use crate::http::AppState;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use malbox_database::PgPool;
use malbox_plugin_internal::registry::manifest::PluginTypeConfig;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/plugins", get(list_plugins))
        .route("/v1/plugins/available", get(available_plugins))
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

#[derive(Deserialize)]
struct AvailablePluginsQuery {
    platform: Option<String>,
}

#[derive(Serialize)]
struct HostPluginInfo {
    name: String,
    version: String,
    description: Option<String>,
    execution: String,
}

#[derive(Serialize)]
struct GuestPluginInfo {
    name: String,
    version: String,
    description: Option<String>,
    execution: String,
    provisioned: bool,
    snapshot_ids: Vec<Uuid>,
}

#[derive(Serialize)]
struct AvailablePluginsResponse {
    host: Vec<HostPluginInfo>,
    guest: Vec<GuestPluginInfo>,
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

async fn available_plugins(
    State(state): State<AppState>,
    Query(query): Query<AvailablePluginsQuery>,
) -> impl IntoResponse {
    let registry_snapshot = state.plugin_registry.snapshot();

    let host: Vec<HostPluginInfo> = registry_snapshot
        .by_type(PluginTypeConfig::Host)
        .into_iter()
        .map(|e| HostPluginInfo {
            name: e.manifest.plugin.name.clone(),
            version: e.manifest.plugin.version.clone(),
            description: e.manifest.plugin.description.clone(),
            execution: format!("{:?}", e.manifest.runtime.execution).to_lowercase(),
        })
        .collect();

    let provisioned_map: std::collections::HashMap<String, Vec<Uuid>> =
        match fetch_provisioned_guest_plugins(&state.pool, query.platform.as_deref()).await {
            Ok(m) => m,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"error": e.to_string()})),
                )
                    .into_response();
            }
        };

    let guest: Vec<GuestPluginInfo> = registry_snapshot
        .by_type(PluginTypeConfig::Guest)
        .into_iter()
        .map(|e| {
            let name = &e.manifest.plugin.name;
            let snapshot_ids = provisioned_map
                .get(name.as_str())
                .cloned()
                .unwrap_or_default();
            GuestPluginInfo {
                name: name.clone(),
                version: e.manifest.plugin.version.clone(),
                description: e.manifest.plugin.description.clone(),
                execution: format!("{:?}", e.manifest.runtime.execution).to_lowercase(),
                provisioned: !snapshot_ids.is_empty(),
                snapshot_ids,
            }
        })
        .collect();

    (
        StatusCode::OK,
        Json(AvailablePluginsResponse { host, guest }),
    )
        .into_response()
}

async fn fetch_provisioned_guest_plugins(
    pool: &PgPool,
    platform: Option<&str>,
) -> std::result::Result<std::collections::HashMap<String, Vec<Uuid>>, sqlx::Error> {
    let rows: Vec<(Uuid, serde_json::Value)> = if let Some(plat) = platform {
        sqlx::query_as(
            r#"
            SELECT ms.id, ms.guest_plugins
            FROM machine_snapshots ms
            JOIN machines m ON ms.machine_id = m.id
            WHERE m.platform::text = $1
              AND ms.guest_plugins IS NOT NULL
              AND ms.guest_plugins != '[]'::jsonb
            "#,
        )
        .bind(plat)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            r#"
            SELECT id, guest_plugins
            FROM machine_snapshots
            WHERE guest_plugins IS NOT NULL
              AND guest_plugins != '[]'::jsonb
            "#,
        )
        .fetch_all(pool)
        .await?
    };

    let mut map: std::collections::HashMap<String, Vec<Uuid>> = std::collections::HashMap::new();
    for (snapshot_id, plugins_json) in rows {
        if let Ok(names) = serde_json::from_value::<Vec<String>>(plugins_json) {
            for name in names {
                map.entry(name).or_default().push(snapshot_id);
            }
        }
    }
    Ok(map)
}
