//! Guest plugin runtime - exposes a [`GuestPlugin`] as a gRPC server.
//!
//! Guest plugins run inside a VM. The daemon is the gRPC client; this
//! runtime spins up the matching server using the [`GuestPlugin`] trait
//! implementation.
//!
//! Module layout:
//! - [`mod@self`] - `GuestRuntime`, `GuestRuntimeConfig` (public API)
//! - [`files`] - file push/pull and path-traversal protection
//! - [`stream`] - result streaming + log entry conversion
//! - [`collector`] - auto-collection logic

pub(crate) mod collector;
mod files;
mod stream;

use collector::AutoCollectSection;

use crate::error::{Result, SdkError};
use crate::log::LogBus;
use crate::plugin::guest::GuestPlugin;
use crate::stash::{ResultStash, StashConfig};

use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::plugin::{
    FileChunkStream, GuestPluginService, GuestPluginServiceServer, LogEntryStream,
    ResultChunkStream, TaskResultStream,
};

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

/// Auto-collection settings for a single directory (artifacts or external logs).
///
/// These values are baked into the plugin binary at compile time by the
/// `#[guest_plugin]` macro, which reads them from `plugin.toml`.
#[derive(Debug, Clone, Copy)]
pub struct AutoCollectRuntimeConfig {
    /// Whether auto-collection is enabled for this directory.
    pub enabled: bool,
    /// Glob patterns for files to include (e.g. `["**/*.log"]`).
    pub include: &'static [&'static str],
    /// Glob patterns for files to exclude.
    pub exclude: &'static [&'static str],
    /// Skip files larger than this (bytes). Prevents collecting huge dumps.
    pub max_file_size: u64,
}

/// Configuration for the guest plugin runtime.
///
/// All values are baked into the plugin binary at compile time by the
/// `#[guest_plugin]` macro (sourced from `plugin.toml`). Paths use
/// `&'static str` so the config can be a `const`.
#[derive(Debug, Clone, Copy)]
pub struct GuestRuntimeConfig {
    /// Address the gRPC server binds to (e.g. `0.0.0.0:50100`).
    pub listen_addr: SocketAddr,
    /// Directory where the daemon pushes sample files.
    pub sample_dir: &'static str,
    /// Directory where plugins write artifact files for collection.
    pub artifact_dir: &'static str,
    /// Directory used for the disk-backed result stash.
    pub stash_dir: &'static str,
    /// Directory for log overflow files when the ring buffer is full.
    pub log_dir: &'static str,
    /// Directory for external log files collected after analysis.
    pub external_log_dir: &'static str,
    /// Results larger than this (bytes) are stashed to disk instead of sent inline.
    pub stash_threshold_bytes: usize,
    /// How long (seconds) a stashed result can remain un-pulled before cleanup.
    pub stash_ttl_secs: u64,
    /// `tracing` filter directive (e.g. `"info"` or `"my_plugin=debug"`).
    pub log_filter: &'static str,
    /// Default analysis timeout in seconds if the daemon does not specify one.
    pub analysis_timeout: u64,
    /// Auto-collection settings for the artifact directory.
    pub auto_collect_artifacts: AutoCollectRuntimeConfig,
    /// Auto-collection settings for the external log directory.
    pub auto_collect_external_logs: AutoCollectRuntimeConfig,
}

/// Sweep log overflow files older than 10 minutes. Called once on runtime
/// startup to clean up orphans from prior crashed runs.
pub fn sweep_log_overflow_orphans(log_dir: &std::path::Path) {
    use std::time::{Duration, SystemTime};

    let grace = Duration::from_secs(10 * 60);
    let now = SystemTime::now();

    let read_dir = match std::fs::read_dir(log_dir) {
        Ok(d) => d,
        Err(_) => return,
    };

    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !name.ends_with(".overflow.jsonl") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
        if age > grace {
            let _ = std::fs::remove_file(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// GuestService - implements GuestPluginService (tonic trait) directly
// ---------------------------------------------------------------------------

/// Internal gRPC service implementation that delegates RPCs to a [`GuestPlugin`].
struct GuestService<P: Send + Sync + 'static> {
    plugin: Arc<P>,
    shutdown_tx: std::sync::Mutex<Option<oneshot::Sender<()>>>,
    sample_dir: PathBuf,
    artifact_dir: PathBuf,
    external_log_dir: PathBuf,
    log_bus: Arc<LogBus>,
    stash: Arc<ResultStash>,
    auto_collect_artifacts: AutoCollectSection,
    auto_collect_external_logs: AutoCollectSection,
    default_timeout: u64,
}

#[tonic::async_trait]
impl<P: GuestPlugin> GuestPluginService for GuestService<P> {
    type ExecuteTaskStream = TaskResultStream;
    type PullFileStream = FileChunkStream;
    type StreamLogsStream = LogEntryStream;
    type PullResultStream = ResultChunkStream;

    // -- Lifecycle --------------------------------------------------------

    async fn initialize(
        &self,
        _request: Request<proto::InitializeRequest>,
    ) -> std::result::Result<Response<proto::InitializeResponse>, Status> {
        Ok(Response::new(proto::InitializeResponse {
            success: true,
            error_message: String::new(),
            capabilities: vec![],
        }))
    }

    async fn health_check(
        &self,
        _request: Request<proto::HealthCheckRequest>,
    ) -> std::result::Result<Response<proto::HealthCheckResponse>, Status> {
        let plugin = self.plugin.clone();
        let (ready, reason) = tokio::task::spawn_blocking(move || {
            let status = plugin.health_check();
            (status.is_ready(), status.reason().to_owned())
        })
        .await
        .unwrap_or((false, "health check panicked".to_string()));
        Ok(Response::new(proto::HealthCheckResponse { ready, reason }))
    }

    async fn shutdown(
        &self,
        _request: Request<proto::ShutdownRequest>,
    ) -> std::result::Result<Response<proto::ShutdownResponse>, Status> {
        if let Ok(mut guard) = self.shutdown_tx.lock()
            && let Some(tx) = guard.take()
        {
            let _ = tx.send(());
        }
        Ok(Response::new(proto::ShutdownResponse {
            acknowledged: true,
        }))
    }

    // -- Task execution ---------------------------------------------------

    async fn execute_task(
        &self,
        request: Request<proto::TaskRequest>,
    ) -> std::result::Result<Response<Self::ExecuteTaskStream>, Status> {
        let req = request.into_inner();
        let (proto_tx, proto_rx) =
            mpsc::channel::<std::result::Result<proto::TaskResult, Status>>(32);
        let (result_tx, mut result_rx) =
            mpsc::channel::<crate::context::message::TaskResultMessage>(32);

        let plugin = self.plugin.clone();
        let full_sample_path = if std::path::Path::new(&req.sample_path).is_relative()
            && !req.sample_path.is_empty()
        {
            self.sample_dir.join(&req.sample_path)
        } else {
            PathBuf::from(&req.sample_path)
        };

        let stash = Arc::clone(&self.stash);
        let artifact_dir = self.artifact_dir.clone();
        let external_log_dir = self.external_log_dir.clone();
        let auto_collect_artifacts = self.auto_collect_artifacts.clone();
        let auto_collect_external_logs = self.auto_collect_external_logs.clone();
        let default_timeout = self.default_timeout;

        // Relay: convert internal messages to proto and forward to tonic stream
        let relay_stash = Arc::clone(&stash);
        let relay_tx = proto_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = result_rx.recv().await {
                let proto_msg = stream::message_to_proto(msg, &relay_stash);
                if relay_tx.send(Ok(proto_msg)).await.is_err() {
                    break;
                }
            }
        });

        tokio::task::spawn_blocking(move || {
            stream::guest_linear_task(
                plugin,
                stream::TaskExecution {
                    task_id: req.task_id,
                    sample_path: full_sample_path,
                    config: req.config,
                    result_tx,
                    proto_tx,
                    stash,
                    artifact_dir,
                    external_log_dir,
                    auto_collect_artifacts,
                    auto_collect_external_logs,
                    default_timeout,
                },
            );
        });

        let stream: TaskResultStream = Box::pin(ReceiverStream::new(proto_rx));
        Ok(Response::new(stream))
    }

    // -- File transfer ----------------------------------------------------

    async fn push_file(
        &self,
        request: Request<tonic::Streaming<proto::FileChunk>>,
    ) -> std::result::Result<Response<proto::FileTransferResponse>, Status> {
        let mut streaming = request.into_inner();
        let mut dest = String::new();
        let mut data = Vec::new();
        while let Some(chunk) = streaming
            .message()
            .await
            .map_err(|e| Status::internal(format!("stream error: {}", e)))?
        {
            if dest.is_empty() {
                dest.clone_from(&chunk.path);
            }
            data.extend_from_slice(&chunk.data);
        }
        match files::push_file(&self.sample_dir, &dest, data).await {
            Ok(()) => Ok(Response::new(proto::FileTransferResponse {
                success: true,
                error_message: String::new(),
            })),
            Err(error_message) => Ok(Response::new(proto::FileTransferResponse {
                success: false,
                error_message,
            })),
        }
    }

    async fn pull_file(
        &self,
        request: Request<proto::PullFileRequest>,
    ) -> std::result::Result<Response<Self::PullFileStream>, Status> {
        let req = request.into_inner();
        match files::pull_file(&self.artifact_dir, &req.path).await {
            Ok(data) => {
                let path = req.path;
                let chunks: Vec<std::result::Result<proto::FileChunk, Status>> = data
                    .chunks(64 * 1024)
                    .enumerate()
                    .map(|(i, chunk)| {
                        let remaining = data.len() - (i * 64 * 1024) - chunk.len();
                        Ok(proto::FileChunk {
                            path: path.clone(),
                            data: chunk.to_vec(),
                            is_last: remaining == 0,
                        })
                    })
                    .collect();
                let stream: Self::PullFileStream = Box::pin(tokio_stream::iter(chunks));
                Ok(Response::new(stream))
            }
            Err(e) => Err(Status::internal(e)),
        }
    }

    // -- Command execution (no-op) ----------------------------------------

    async fn execute_command(
        &self,
        _request: Request<proto::ExecRequest>,
    ) -> std::result::Result<Response<proto::ExecResponse>, Status> {
        Ok(Response::new(proto::ExecResponse {
            exit_code: None,
            stdout: vec![],
            stderr: vec![],
            pid: None,
        }))
    }

    // -- Log streaming ----------------------------------------------------

    async fn stream_logs(
        &self,
        request: Request<proto::LogStreamRequest>,
    ) -> std::result::Result<Response<Self::StreamLogsStream>, Status> {
        let req = request.into_inner();
        let log_bus = Arc::clone(&self.log_bus);
        let s = async_stream::stream! {
            if req.include_buffered {
                for entry in log_bus.drain_atomic() {
                    yield Ok(stream::log_entry_to_proto(entry));
                }
            }
            loop {
                let entries = log_bus.recv_atomic().await;
                if entries.is_empty() { break; }
                for entry in entries {
                    yield Ok(stream::log_entry_to_proto(entry));
                }
            }
        };
        Ok(Response::new(Box::pin(s)))
    }

    // -- Large result pull ------------------------------------------------

    async fn pull_result(
        &self,
        request: Request<proto::PullResultRequest>,
    ) -> std::result::Result<Response<Self::PullResultStream>, Status> {
        let req = request.into_inner();
        let entry = self
            .stash
            .take(&req.handle)
            .ok_or_else(|| Status::not_found(format!("stash handle not found: {}", req.handle)))?;
        let file = tokio::fs::File::open(&entry.path)
            .await
            .map_err(|e| Status::internal(format!("failed to open stashed result: {e}")))?;
        let stash = Arc::clone(&self.stash);
        let s = async_stream::stream! {
            use tokio::io::AsyncReadExt;
            let mut file = file;
            let mut buf = vec![0u8; 64 * 1024];
            let mut index: u32 = 0;
            loop {
                let n = match file.read(&mut buf).await {
                    Ok(n) => n,
                    Err(e) => {
                        yield Err(tonic::Status::internal(format!("read error: {e}")));
                        stash.cleanup_after_pull(&entry);
                        return;
                    }
                };
                if n == 0 {
                    yield Ok(proto::ResultChunk { data: vec![], index, is_last: true });
                    break;
                }
                yield Ok(proto::ResultChunk { data: buf[..n].to_vec(), index, is_last: false });
                index += 1;
            }
            stash.cleanup_after_pull(&entry);
        };
        Ok(Response::new(Box::pin(s)))
    }
}

// ---------------------------------------------------------------------------
// GuestRuntime - public API
// ---------------------------------------------------------------------------

/// Runtime for [`GuestPlugin`] implementations.
///
/// Runs a linear lifecycle per task: `on_start` -> `execute_sample` -> wait
/// -> `on_stop`, with auto-collection of artifacts and external logs.
pub struct GuestRuntime<P: Send + Sync + 'static> {
    plugin: Arc<P>,
    config: GuestRuntimeConfig,
    log_bus: Option<Arc<LogBus>>,
}

impl<P: GuestPlugin> GuestRuntime<P> {
    /// Create a guest runtime with custom configuration.
    pub fn with_config(plugin: P, config: GuestRuntimeConfig) -> Self {
        Self {
            plugin: Arc::new(plugin),
            config,
            log_bus: None,
        }
    }

    /// Attach an externally-created [`LogBus`] so the gRPC log stream uses
    /// the same bus that the tracing layer / C++ FFI already writes to.
    pub fn with_log_bus(mut self, bus: Arc<LogBus>) -> Self {
        self.log_bus = Some(bus);
        self
    }

    /// Start the gRPC server and block until shutdown.
    pub async fn run(self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let sample_dir: PathBuf = PathBuf::from(self.config.sample_dir);
        let artifact_dir: PathBuf = PathBuf::from(self.config.artifact_dir);
        let stash_dir: PathBuf = PathBuf::from(self.config.stash_dir);
        let log_dir: PathBuf = PathBuf::from(self.config.log_dir);
        let external_log_dir: PathBuf = PathBuf::from(self.config.external_log_dir);

        // Ensure all runtime directories exist.
        for (label, dir) in [
            ("sample_dir", &sample_dir),
            ("artifact_dir", &artifact_dir),
            ("stash_dir", &stash_dir),
            ("log_dir", &log_dir),
            ("external_log_dir", &external_log_dir),
        ] {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| SdkError::Init(format!("failed to create {}: {}", label, e)))?;
        }

        // Set up the result stash for large payloads. On startup, sweep any
        // orphan files left behind by a prior crashed run.
        if let Err(e) = ResultStash::sweep_orphans_on_startup(&stash_dir) {
            warn!(error = %e, "failed to sweep result stash orphans on startup");
        }
        let stash_config = StashConfig {
            threshold_bytes: self.config.stash_threshold_bytes,
            ttl: std::time::Duration::from_secs(self.config.stash_ttl_secs),
        };
        let stash = Arc::new(
            ResultStash::new(stash_dir, stash_config)
                .map_err(|e| SdkError::Init(format!("failed to create stash: {}", e)))?,
        );

        // Spawn a background TTL sweeper: purges stash entries that were
        // never pulled by the daemon within the configured TTL window.
        {
            let sweep_stash = Arc::clone(&stash);
            let sweep_interval = sweep_stash.config().ttl / 2;
            tokio::spawn(async move {
                let mut ticker = tokio::time::interval(sweep_interval);
                // Skip the immediate first tick.
                ticker.tick().await;
                loop {
                    ticker.tick().await;
                    let n = sweep_stash.sweep_expired();
                    if n > 0 {
                        warn!(
                            reclaimed = n,
                            "TTL sweep reclaimed stale result stash entries"
                        );
                    }
                }
            });
        }

        // Use the externally-provided LogBus (C++ FFI path where tracing
        // was already initialised with this bus), or create a fresh one and
        // install the tracing layer ourselves (Rust SDK path).
        let log_bus = match self.log_bus {
            Some(bus) => bus,
            None => {
                sweep_log_overflow_orphans(&log_dir);
                let overflow_path =
                    log_dir.join(format!("run-{}.overflow.jsonl", std::process::id()));

                let bus = Arc::new(LogBus::with_overflow(1024, overflow_path));
                crate::internal::init_tracing(self.config.log_filter, Some(Arc::clone(&bus)));
                bus
            }
        };

        let to_section = |cfg: &AutoCollectRuntimeConfig| AutoCollectSection {
            enabled: cfg.enabled,
            include: cfg.include.iter().map(|s| (*s).to_string()).collect(),
            exclude: cfg.exclude.iter().map(|s| (*s).to_string()).collect(),
            max_file_size: cfg.max_file_size,
        };

        let service = GuestService {
            plugin: self.plugin,
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
            sample_dir,
            artifact_dir,
            external_log_dir,
            log_bus: Arc::clone(&log_bus),
            stash: Arc::clone(&stash),
            auto_collect_artifacts: to_section(&self.config.auto_collect_artifacts),
            auto_collect_external_logs: to_section(&self.config.auto_collect_external_logs),
            default_timeout: self.config.analysis_timeout,
        };

        let addr = self.config.listen_addr;

        info!(address = %addr, "Guest plugin gRPC server starting");

        tonic::transport::Server::builder()
            .add_service(GuestPluginServiceServer::new(service))
            .serve_with_shutdown(addr, async {
                let _ = shutdown_rx.await;
            })
            .await
            .map_err(|e| {
                let mut msg = format!("gRPC server error on {addr}: {e}");
                let mut source = std::error::Error::source(&e);
                while let Some(cause) = source {
                    msg.push_str(&format!("\n  caused by: {cause}"));
                    source = std::error::Error::source(cause);
                }
                SdkError::Init(msg)
            })?;

        info!("Guest plugin gRPC server stopped");
        Ok(())
    }

    /// Start the gRPC server, creating a tokio runtime if needed.
    ///
    /// This is the entry point for `fn main()` style plugins that don't
    /// already have an async runtime.
    pub fn run_blocking(self) -> Result<()> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| SdkError::Init(format!("failed to create tokio runtime: {}", e)))?;
        rt.block_on(self.run())
    }
}

#[cfg(test)]
mod guest_runtime_config_tests {
    use super::*;

    const STATIC_SANITY: GuestRuntimeConfig = GuestRuntimeConfig {
        listen_addr: std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::UNSPECIFIED,
            50100,
        )),
        sample_dir: "/opt/malbox/samples",
        artifact_dir: "/opt/malbox/artifacts",
        stash_dir: "/opt/malbox/stash",
        log_dir: "/opt/malbox/logs",
        external_log_dir: "/opt/malbox/ext-logs",
        stash_threshold_bytes: 1_048_576,
        stash_ttl_secs: 120,
        log_filter: "info",
        analysis_timeout: 120,
        auto_collect_artifacts: AutoCollectRuntimeConfig {
            enabled: true,
            include: &["**/*"],
            exclude: &[],
            max_file_size: 50 * 1024 * 1024,
        },
        auto_collect_external_logs: AutoCollectRuntimeConfig {
            enabled: true,
            include: &["**/*"],
            exclude: &[],
            max_file_size: 50 * 1024 * 1024,
        },
    };

    #[test]
    fn guest_runtime_config_is_const_constructible() {
        // Compilation of STATIC_SANITY above is the actual check.
        assert_eq!(STATIC_SANITY.listen_addr.port(), 50100);
    }
}
