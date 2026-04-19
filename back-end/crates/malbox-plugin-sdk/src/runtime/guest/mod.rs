//! Guest plugin runtime — exposes a `Plugin` as a gRPC server.
//!
//! Guest plugins run inside a VM. The daemon is the gRPC client; this
//! runtime spins up the matching server using the [`Plugin`] trait
//! implementation.
//!
//! Module layout:
//! - [`mod@self`] — `GuestPluginRuntime`, `GuestRuntimeConfig` (public API)
//! - [`bridge`] — `GuestPluginBridge` (translates RPCs to `Plugin` calls)
//! - [`files`] — file push/pull and path-traversal protection
//! - [`exec`] — default command executor + execution sync channel
//! - [`stream`] — result streaming + log entry conversion

mod bridge;
pub mod exec;
mod files;
mod stream;

pub use exec::{ExecutionNotifier, ExecutionWaiter, execution_channel};

use crate::error::{Result, SdkError};
use crate::log::LogBus;
use crate::plugin::Plugin;
use crate::stash::{ResultStash, StashConfig};
use crate::types::PluginMeta;

use bridge::GuestPluginBridge;
use malbox_plugin_transport::plugin::GrpcServer;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::oneshot;
use tracing::{info, warn};

/// Configuration for the guest plugin runtime.
pub struct GuestRuntimeConfig {
    /// Socket address the gRPC server listens on.
    pub listen_addr: SocketAddr,
    /// Working directory for file operations (push_file, pull_file, execute_command cwd).
    /// Created on startup if it doesn't exist.
    pub work_dir: PathBuf,
}

impl Default for GuestRuntimeConfig {
    fn default() -> Self {
        let port: u16 = std::env::var("MALBOX_PLUGIN_PORT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50051);

        let work_dir = std::env::var("MALBOX_WORK_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/tmp/malbox"));

        Self {
            listen_addr: ([0, 0, 0, 0], port).into(),
            work_dir,
        }
    }
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

/// Runtime that exposes a [`Plugin`] as a gRPC server.
///
/// The daemon connects to this server to initialize, execute tasks, send
/// events, and request shutdown. Plugin trait methods are called from
/// blocking threads via `spawn_blocking`.
pub struct GuestPluginRuntime<P: Send + Sync + 'static> {
    plugin: Arc<P>,
    config: GuestRuntimeConfig,
    log_bus: Option<Arc<LogBus>>,
}

impl<P: Plugin> GuestPluginRuntime<P> {
    /// Create a guest runtime for the given plugin and metadata.
    ///
    /// Used by generated `main()` from `#[malbox::guest_plugin]`.
    pub fn new(plugin: P, _meta: PluginMeta) -> Self {
        Self::with_config(plugin, GuestRuntimeConfig::default())
    }

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

        // Ensure work directory exists.
        tokio::fs::create_dir_all(&self.config.work_dir)
            .await
            .map_err(|e| SdkError::Init(format!("failed to create work_dir: {}", e)))?;

        // Set up the result stash for large payloads. On startup, sweep any
        // orphan files left behind by a prior crashed run.
        let stash_dir = self.config.work_dir.join("_stash");
        if let Err(e) = ResultStash::sweep_orphans_on_startup(&stash_dir) {
            warn!(error = %e, "failed to sweep result stash orphans on startup");
        }
        let stash_config = {
            let threshold = std::env::var("MALBOX_STASH_THRESHOLD")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1024 * 1024);
            let ttl_secs = std::env::var("MALBOX_STASH_TTL")
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(120);
            StashConfig {
                threshold_bytes: threshold,
                ttl: std::time::Duration::from_secs(ttl_secs),
            }
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
                // Overflow file lives under work_dir/_logs/ so orphan sweep
                // can identify old files.
                let log_dir = std::env::var("MALBOX_LOG_OVERFLOW_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| self.config.work_dir.join("_logs"));
                if let Err(e) = std::fs::create_dir_all(&log_dir) {
                    warn!(error = %e, "failed to create log overflow dir");
                }
                sweep_log_overflow_orphans(&log_dir);
                let overflow_path =
                    log_dir.join(format!("run-{}.overflow.jsonl", std::process::id()));

                let bus = Arc::new(LogBus::with_overflow(1024, overflow_path));
                crate::internal::init_tracing(Some(Arc::clone(&bus)));
                bus
            }
        };

        let handler = GuestPluginBridge {
            plugin: self.plugin,
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
            work_dir: self.config.work_dir,
            execution_notifiers: std::sync::Mutex::new(HashMap::new()),
            last_execution: std::sync::Mutex::new(None),
            log_bus: Arc::clone(&log_bus),
            stash: Arc::clone(&stash),
        };

        let server = GrpcServer::new(handler);
        let addr = self.config.listen_addr;

        info!(address = %addr, "Guest plugin gRPC server starting");

        tonic::transport::Server::builder()
            .add_service(server.into_service())
            .serve_with_shutdown(addr, async {
                let _ = shutdown_rx.await;
            })
            .await
            .map_err(|e| SdkError::Init(format!("gRPC server error: {}", e)))?;

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
