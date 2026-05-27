use axum::{Router, http::StatusCode, response::IntoResponse, routing::get};
use malbox_config::Config as MalboxConfig;
use malbox_database::{PgPool, repositories::tasks::Task};
use malbox_plugin_internal::registry::PluginRegistry;
use malbox_resources::MachinePool;
use malbox_scheduler::TaskCancellationRegistry;
use malbox_utils::SampleStore;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

mod error;
mod images;
mod machines;
mod plugins;
mod recipes;
mod samples;
mod tasks;
mod transforms;

pub use error::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone)]
struct AppState {
    config: MalboxConfig,
    pool: PgPool,
    task_tx: mpsc::Sender<Task>,
    sample_store: Arc<SampleStore>,
    machine_pool: Arc<MachinePool>,
    plugin_registry: Arc<PluginRegistry>,
    cancel_registry: Arc<TaskCancellationRegistry>,
}

#[allow(clippy::too_many_arguments)]
pub async fn serve(
    conf: MalboxConfig,
    db: PgPool,
    task_tx: mpsc::Sender<Task>,
    sample_store: Arc<SampleStore>,
    machine_pool: Arc<MachinePool>,
    plugin_registry: Arc<PluginRegistry>,
    cancel_registry: Arc<TaskCancellationRegistry>,
    shutdown_token: CancellationToken,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let shared_state = AppState {
        config: conf,
        pool: db,
        task_tx,
        sample_store,
        machine_pool,
        plugin_registry,
        cancel_registry,
    };

    let app = api_router()
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

    let address = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|e| format!("error binding TcpListener: {}", e))?;

    info!(address = %address, "HTTP server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move { shutdown_token.cancelled().await })
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
}

fn api_router() -> Router<AppState> {
    Router::new()
        .route("/", get(root))
        .fallback(handler_404)
        .merge(tasks::cancel::router())
        .merge(tasks::create::router())
        .merge(tasks::get::router())
        .merge(tasks::report::router())
        .merge(tasks::results::router())
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

async fn root() -> &'static str {
    "Server is running!"
}

async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "The requested resource was not found",
    )
}
