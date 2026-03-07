use axum::{
    Router,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use malbox_config::Config as MalboxConfig;
use malbox_database::{PgPool, repositories::tasks::Task};
use malbox_utils::SampleStore;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tower_http::trace::TraceLayer;

mod error;
mod tasks;

pub use error::Error;
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Clone)]
struct AppState {
    config: MalboxConfig,
    pool: PgPool,
    task_tx: mpsc::Sender<Task>,
    sample_store: Arc<SampleStore>,
}

pub async fn serve(conf: MalboxConfig, db: PgPool, task_tx: mpsc::Sender<Task>, sample_store: Arc<SampleStore>) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let shared_state = AppState {
        config: conf,
        pool: db,
        task_tx,
        sample_store,
    };

    let app = api_router()
        .layer(TraceLayer::new_for_http())
        .with_state(shared_state.clone());

    let host = shared_state.config.http.host;
    let port = shared_state.config.http.port;

    let address = format!("{}:{}", host, port);
    let listener = TcpListener::bind(&address)
        .await
        .map_err(|e| format!("error binding TcpListener: {}", e))?;

    tracing::info!("[STARTUP] listening on http://{}", address);

    axum::serve(listener, app)
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
}

fn api_router() -> Router<AppState> {
    Router::new()
        .route("/", get(root))
        .fallback(handler_404)
        .merge(tasks::create::router())
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
