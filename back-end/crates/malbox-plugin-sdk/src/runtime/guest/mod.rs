//! Guest plugin runtime — exposes a `HostPlugin` as a gRPC server.
//!
//! Guest plugins run inside a VM. The daemon is the gRPC client; this
//! runtime spins up the matching server using the [`HostPlugin`] trait
//! implementation.
//!
//! Module layout:
//! - [`mod@self`] — `GuestPluginRuntime`, `GuestRuntimeConfig` (public API)
//! - [`bridge`] — `GuestPluginBridge` (translates RPCs to `HostPlugin` calls)
//! - [`files`] — file push/pull and path-traversal protection
//! - [`exec`] — default command executor + execution sync channel
//! - [`stream`] — result streaming + log entry conversion

mod bridge;
pub(crate) mod collector;
pub mod exec;
mod files;
mod stream;

use collector::AutoCollectSection;

use crate::error::{Result, SdkError};
use crate::guest_plugin::GuestPlugin;
use crate::log::LogBus;
use crate::plugin::HostPlugin;
use crate::stash::{ResultStash, StashConfig};

use bridge::{GuestLinearBridge, GuestPluginBridge};
use malbox_plugin_transport::plugin::GrpcServer;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::oneshot;
use tracing::{info, warn};

/// Configuration for the guest plugin runtime.
///
/// Values are baked into the plugin binary at compile time (by the
/// `#[guest_plugin]` macro, which reads them from `plugin.toml`). Paths are
/// `&'static str` so a `const` can be constructed at macro expansion time.
/// Compile-time auto-collection settings for a single directory.
#[derive(Debug, Clone, Copy)]
pub struct AutoCollectRuntimeConfig {
    pub enabled: bool,
    pub include: &'static [&'static str],
    pub exclude: &'static [&'static str],
    pub max_file_size: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct GuestRuntimeConfig {
    pub listen_addr: SocketAddr,
    pub sample_dir: &'static str,
    pub artifact_dir: &'static str,
    pub stash_dir: &'static str,
    pub log_dir: &'static str,
    pub external_log_dir: &'static str,
    pub stash_threshold_bytes: usize,
    pub stash_ttl_secs: u64,
    pub log_filter: &'static str,
    pub analysis_timeout: u64,
    pub auto_collect_artifacts: AutoCollectRuntimeConfig,
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

/// Runtime that exposes a [`HostPlugin`] as a gRPC server.
///
/// The daemon connects to this server to initialize, execute tasks, send
/// events, and request shutdown. Plugin trait methods are called from
/// blocking threads via `spawn_blocking`.
pub struct GuestPluginRuntime<P: Send + Sync + 'static> {
    plugin: Arc<P>,
    config: GuestRuntimeConfig,
    log_bus: Option<Arc<LogBus>>,
}

impl<P: HostPlugin> GuestPluginRuntime<P> {
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

        let handler = GuestPluginBridge {
            plugin: self.plugin,
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
            sample_dir,
            artifact_dir,
            external_log_dir,
            execution_notifiers: std::sync::Mutex::new(HashMap::new()),
            last_execution: std::sync::Mutex::new(None),
            log_bus: Arc::clone(&log_bus),
            stash: Arc::clone(&stash),
            auto_collect_artifacts: to_section(&self.config.auto_collect_artifacts),
            auto_collect_external_logs: to_section(&self.config.auto_collect_external_logs),
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

/// Runtime for [`GuestPlugin`] implementations with a linear lifecycle.
///
/// Unlike [`GuestPluginRuntime`] (which dispatches RPCs to `HostPlugin`
/// callbacks via execution channels), this runtime runs a simple linear
/// sequence: `on_start` -> `execute_sample` -> wait -> `on_stop`.
pub struct GuestLinearRuntime<P: Send + Sync + 'static> {
    plugin: Arc<P>,
    config: GuestRuntimeConfig,
    log_bus: Option<Arc<LogBus>>,
}

impl<P: GuestPlugin> GuestLinearRuntime<P> {
    /// Create a guest linear runtime with custom configuration.
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

        let handler = GuestLinearBridge {
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

        let server = GrpcServer::new(handler);
        let addr = self.config.listen_addr;

        info!(address = %addr, "Guest linear plugin gRPC server starting");

        tonic::transport::Server::builder()
            .add_service(server.into_service())
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

        info!("Guest linear plugin gRPC server stopped");
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
