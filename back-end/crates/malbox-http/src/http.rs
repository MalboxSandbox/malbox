use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use malbox_config::Config as MalboxConfig;
use malbox_database::PgPool;
use malbox_plugin_internal::registry::PluginRegistry;
use malbox_resources::MachinePool;
use malbox_scheduler::OrchestratorHandle;
use malbox_utils::SampleStore;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info, warn};

mod dto;
mod error;
mod images;
mod machines;
mod plugins;
mod recipes;
pub(crate) mod report_service;
mod samples;
mod tasks;
mod transforms;

pub use error::Error;
/// Result alias for HTTP handlers, defaulting to the API [`Error`].
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone)]
struct AppState {
    config: MalboxConfig,
    pool: PgPool,
    orchestrator: OrchestratorHandle,
    sample_store: Arc<SampleStore>,
    machine_pool: Arc<MachinePool>,
    plugin_registry: Arc<PluginRegistry>,
}

/// Run the HTTP API server until `shutdown_token` is cancelled.
///
/// Also serves the front-end SPA when `http.web_dir` is configured.
///
/// # Errors
///
/// Fails if the listener cannot bind to the configured host and port, or if
/// the server itself errors while running.
pub async fn serve(
    conf: MalboxConfig,
    db: PgPool,
    orchestrator: OrchestratorHandle,
    sample_store: Arc<SampleStore>,
    machine_pool: Arc<MachinePool>,
    plugin_registry: Arc<PluginRegistry>,
    shutdown_token: CancellationToken,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let shared_state = AppState {
        config: conf,
        pool: db,
        orchestrator,
        sample_store,
        machine_pool,
        plugin_registry,
    };

    let app = api_router();

    // Serve the front-end SPA from the same origin as the API when configured.
    // The `/v1` API routes are matched first; any other path falls back to the
    // static bundle, and unknown paths return index.html so the SPA's
    // client-side router can resolve deep links.
    let app = match shared_state.config.http.web_dir.as_deref() {
        Some(dir) if Path::new(dir).is_dir() => {
            let dir = PathBuf::from(dir);
            let index = dir.join("index.html");
            info!(web_dir = %dir.display(), "serving front-end SPA");
            app.fallback_service(ServeDir::new(dir).not_found_service(ServeFile::new(index)))
        }
        Some(dir) => {
            warn!(
                web_dir = %dir,
                "configured http.web_dir does not exist; serving API only"
            );
            app.fallback(handler_404)
        }
        None => app.fallback(handler_404),
    };

    let app = app
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(
                    DefaultMakeSpan::new()
                        .level(Level::INFO)
                        .include_headers(false),
                )
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(shared_state.clone());

    let host = shared_state.config.http.host;
    let port = shared_state.config.http.port;

    let address = format!("{host}:{port}");
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|e| format!("error binding TcpListener: {e}"))?;

    info!(address = %address, "HTTP server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move { shutdown_token.cancelled().await })
        .await
        .map_err(Into::into)
}

fn api_router() -> Router<AppState> {
    Router::new()
        .route("/healthz", get(health))
        .merge(tasks::cancel::router())
        .merge(tasks::create::router())
        .merge(tasks::get::router())
        .merge(tasks::report::router())
        .merge(tasks::artifacts::router())
        .merge(samples::router())
        .merge(images::router())
        .merge(machines::router())
        .merge(plugins::router())
        .merge(recipes::create::router())
        .merge(recipes::get::router())
        .merge(recipes::update::router())
        .merge(recipes::delete::router())
        .merge(transforms::create::router())
        .merge(transforms::get::router())
        .merge(transforms::update::router())
        .merge(transforms::delete::router())
}

async fn health() -> &'static str {
    "ok"
}

async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "The requested resource was not found",
    )
}
