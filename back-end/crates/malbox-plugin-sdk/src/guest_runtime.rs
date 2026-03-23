//! Guest plugin runtime — bridges internal handler traits to a gRPC server.
//!
//! Unlike the IPC-based host runtime which polls for events, the guest
//! runtime is reactive: the daemon calls RPCs on the plugin's gRPC server,
//! and each RPC handler delegates to the user's handler trait implementations
//! via `tokio::task::spawn_blocking`.

use crate::context::Context;
use crate::error::{Result, SdkError};
use crate::internal::{
    DaemonEventHandler, PluginEventHandler, SampleEventHandler, StartHandler, StopHandler,
    TaskEventHandler, TaskHandler,
};
use crate::types::Task;

use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::messages::events::*;
use malbox_plugin_transport::plugin::{GrpcEmitter, GrpcServer, GuestPluginHandler};

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tonic::Status;
use tracing::{error, info};

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

        Self {
            listen_addr: ([0, 0, 0, 0], port).into(),
            work_dir: PathBuf::from("/tmp/malbox"),
        }
    }
}

/// Runtime that exposes a plugin as a gRPC server using internal handler traits.
///
/// The daemon connects to this server to initialize, execute tasks, send
/// events, and request shutdown. All handler trait methods are called from
/// blocking threads via `spawn_blocking`.
pub struct GuestPluginRuntime<P: Send + Sync + 'static> {
    plugin: Arc<P>,
    config: GuestRuntimeConfig,
}

impl<P> GuestPluginRuntime<P>
where
    P: TaskHandler
        + StartHandler
        + StopHandler
        + DaemonEventHandler
        + TaskEventHandler
        + PluginEventHandler
        + SampleEventHandler
        + Send
        + Sync
        + 'static,
{
    /// Create a guest runtime with default config (listens on `0.0.0.0:50051`).
    pub fn new(plugin: P) -> Self {
        Self::with_config(plugin, GuestRuntimeConfig::default())
    }

    /// Create a guest runtime with custom config.
    pub fn with_config(plugin: P, config: GuestRuntimeConfig) -> Self {
        Self {
            plugin: Arc::new(plugin),
            config,
        }
    }

    /// Create a guest runtime using the new internal handler traits.
    /// Used by generated `main()` from `#[malbox::guest_plugin]`.
    pub fn new_v2(plugin: P, _meta: crate::types::PluginMeta) -> Self {
        Self {
            plugin: Arc::new(plugin),
            config: GuestRuntimeConfig::default(),
        }
    }

    /// Start the gRPC server and block until shutdown.
    pub async fn run(self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Ensure work directory exists
        tokio::fs::create_dir_all(&self.config.work_dir)
            .await
            .map_err(|e| SdkError::Init(format!("failed to create work_dir: {}", e)))?;

        let handler = GuestPluginBridge {
            plugin: self.plugin,
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
            work_dir: self.config.work_dir,
        };

        let server = GrpcServer::new(handler);
        let addr = self.config.listen_addr;

        info!("Guest plugin gRPC server starting on {}", addr);

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

// ---------------------------------------------------------------------------
// Bridge: GuestPluginHandler -> Internal handler traits
// ---------------------------------------------------------------------------

struct GuestPluginBridge<P: Send + Sync + 'static> {
    plugin: Arc<P>,
    shutdown_tx: std::sync::Mutex<Option<oneshot::Sender<()>>>,
    work_dir: PathBuf,
}

#[tonic::async_trait]
impl<P> GuestPluginHandler for GuestPluginBridge<P>
where
    P: TaskHandler
        + StartHandler
        + StopHandler
        + DaemonEventHandler
        + TaskEventHandler
        + PluginEventHandler
        + SampleEventHandler
        + Send
        + Sync
        + 'static,
{
    async fn on_initialize(
        &self,
        _plugin_id: i32,
        config: HashMap<String, String>,
    ) -> std::result::Result<Vec<String>, String> {
        let plugin = self.plugin.clone();
        let result = tokio::task::spawn_blocking(move || plugin.__handle_start(config))
            .await
            .map_err(|e| format!("spawn_blocking panicked: {}", e))?;

        match result {
            Ok(()) => Ok(vec![]),
            Err(e) => Err(e.to_string()),
        }
    }

    async fn on_health_check(&self) -> (bool, String) {
        (true, String::new())
    }

    async fn on_shutdown(&self, _graceful: bool) {
        let plugin = self.plugin.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = plugin.__handle_stop() {
                error!("Plugin on_stop error: {}", e);
            }
        })
        .await;

        // Signal the server to stop
        if let Ok(mut guard) = self.shutdown_tx.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
            }
        }
    }

    async fn on_execute_task(
        &self,
        task_id: i32,
        sample_path: String,
        config: HashMap<String, String>,
        result_tx: mpsc::Sender<std::result::Result<proto::TaskResult, Status>>,
    ) {
        let plugin = self.plugin.clone();
        // Resolve the sample path relative to the work directory so plugin
        // handlers can read it via task.sample_bytes().
        let full_sample_path = if Path::new(&sample_path).is_relative() && !sample_path.is_empty() {
            self.work_dir.join(&sample_path)
        } else {
            PathBuf::from(&sample_path)
        };
        let _ = tokio::task::spawn_blocking(move || {
            let emitter = GrpcEmitter::with_task(task_id, result_tx);
            let ctx = Context::new(&emitter);

            let task = Task::new(task_id, full_sample_path, config);

            match plugin.__handle_task(task, &ctx) {
                Ok(results) => {
                    let total = results.len();
                    for (i, result) in results.into_iter().enumerate() {
                        let is_final = i == total - 1;
                        let (result_name, data, format) = match result {
                            crate::types::PluginResult::Json { name, data } => {
                                (name, data, proto::ResultFormat::Json)
                            }
                            crate::types::PluginResult::Bytes { name, data } => {
                                (name, data, proto::ResultFormat::Bytes)
                            }
                            crate::types::PluginResult::File { name, path } => {
                                match std::fs::read(&path) {
                                    Ok(data) => (name, data, proto::ResultFormat::Bytes),
                                    Err(e) => {
                                        error!(
                                            "Failed to read result file {}: {}",
                                            path.display(),
                                            e
                                        );
                                        continue;
                                    }
                                }
                            }
                        };

                        tracing::debug!(result_name, is_final, "Streaming result back to daemon");

                        let task_result = proto::TaskResult {
                            task_id,
                            result_name,
                            data,
                            format: format.into(),
                            is_final,
                        };

                        if let Err(e) = emitter.send_task_result(task_result) {
                            error!("Failed to stream result: {}", e);
                            break;
                        }
                    }

                    // If there were no results, send a final empty marker so the
                    // daemon-side stream terminates cleanly.
                    if total == 0 {
                        let _ = emitter.send_task_result(proto::TaskResult {
                            task_id,
                            result_name: String::new(),
                            data: vec![],
                            format: proto::ResultFormat::Unspecified.into(),
                            is_final: true,
                        });
                    }
                }
                Err(e) => {
                    error!("Plugin task handler error: {}", e);
                }
            }
            // Channel drops here, closing the stream
        })
        .await;
    }

    async fn on_event(&self, event: Event, payload: Payload) -> std::result::Result<(), String> {
        let plugin = self.plugin.clone();
        let result = tokio::task::spawn_blocking(move || {
            let emitter = GrpcEmitter::noop();
            let ctx = Context::new(&emitter);
            dispatch_event(&*plugin, event, payload, &ctx)
        })
        .await
        .map_err(|e| format!("spawn_blocking panicked: {}", e))?;

        result.map_err(|e| e.to_string())
    }

    async fn on_push_file(&self, dest: &str, data: Vec<u8>) -> std::result::Result<(), String> {
        let path = resolve_path(&self.work_dir, dest)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("failed to create parent dirs: {}", e))?;
        }
        tokio::fs::write(&path, &data)
            .await
            .map_err(|e| format!("failed to write file: {}", e))
    }

    async fn on_pull_file(&self, source: &str) -> std::result::Result<Vec<u8>, String> {
        let path = resolve_path(&self.work_dir, source)?;
        tokio::fs::read(&path)
            .await
            .map_err(|e| format!("failed to read file: {}", e))
    }

    async fn on_execute_command(
        &self,
        command: &str,
        args: &[String],
        cwd: Option<&str>,
        env: HashMap<String, String>,
        timeout_ms: Option<u64>,
        background: bool,
    ) -> std::result::Result<proto::ExecResponse, String> {
        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args).envs(&env);

        if let Some(cwd) = cwd {
            cmd.current_dir(resolve_path(&self.work_dir, cwd)?);
        }

        if background {
            cmd.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            cmd.spawn().map_err(|e| format!("failed to spawn: {}", e))?;
            return Ok(proto::ExecResponse {
                exit_code: None,
                stdout: vec![],
                stderr: vec![],
            });
        }

        let output_fut = cmd.output();
        let output = match timeout_ms {
            Some(ms) => tokio::time::timeout(std::time::Duration::from_millis(ms), output_fut)
                .await
                .map_err(|_| "command timed out".to_string())?
                .map_err(|e| format!("command failed: {}", e))?,
            None => output_fut
                .await
                .map_err(|e| format!("command failed: {}", e))?,
        };

        Ok(proto::ExecResponse {
            exit_code: output.status.code(),
            stdout: output.stdout,
            stderr: output.stderr,
        })
    }
}

/// Dispatch an event to the appropriate handler trait method.
fn dispatch_event<P>(plugin: &P, event: Event, payload: Payload, ctx: &Context) -> Result<()>
where
    P: TaskHandler
        + StartHandler
        + StopHandler
        + DaemonEventHandler
        + TaskEventHandler
        + PluginEventHandler
        + SampleEventHandler,
{
    match (event, payload) {
        (Event::Task(task_event), Payload::Task(task_payload)) => {
            if matches!(task_event, TaskEvent::TaskStarting | TaskEvent::TaskCreated) {
                let task = Task::new(task_payload.task_id, PathBuf::new(), HashMap::new());
                let _results = plugin.__handle_task(task, ctx)?;
            }
            plugin.__handle_task_lifecycle_event(task_event, task_payload, ctx)
        }
        (Event::Plugin(plugin_event), Payload::Plugin(plugin_payload)) => {
            plugin.__handle_plugin_event(plugin_event, plugin_payload, ctx)
        }
        (Event::Sample(sample_event), Payload::Sample(sample_payload)) => {
            plugin.__handle_sample_event(sample_event, sample_payload, ctx)
        }
        (Event::Daemon(daemon_event), _) => {
            // DaemonShutdown is handled by the on_shutdown RPC, but still
            // forward to the user's handler if they registered one.
            plugin.__handle_daemon_event(daemon_event, ctx)
        }
        _ => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Path resolution helpers (used by file transfer handlers)
// ---------------------------------------------------------------------------

/// Normalize a path by resolving `.` and `..` components without filesystem access.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if matches!(components.last(), Some(Component::Normal(_))) {
                    components.pop();
                }
            }
            Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

/// Resolve a relative path against `work_dir`, rejecting traversal attempts.
fn resolve_path(work_dir: &Path, relative: &str) -> std::result::Result<PathBuf, String> {
    if Path::new(relative).is_absolute() {
        return Err(format!("absolute paths not allowed: {}", relative));
    }

    let joined = work_dir.join(relative);
    let normalized = normalize_path(&joined);
    let normalized_work_dir = normalize_path(work_dir);

    if !normalized.starts_with(&normalized_work_dir) {
        return Err(format!("path escapes work directory: {}", relative));
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: minimal plugin impl for testing the bridge
    struct NoopPlugin;
    impl TaskHandler for NoopPlugin {
        fn __handle_task(
            &self,
            _task: Task,
            _ctx: &Context,
        ) -> Result<Vec<crate::types::PluginResult>> {
            Ok(vec![])
        }
    }
    impl StartHandler for NoopPlugin {
        fn __handle_start(&self, _raw_config: HashMap<String, String>) -> Result<()> {
            Ok(())
        }
    }
    impl StopHandler for NoopPlugin {
        fn __handle_stop(&self) -> Result<()> {
            Ok(())
        }
    }
    impl DaemonEventHandler for NoopPlugin {
        fn __handle_daemon_event(&self, _event: DaemonEvent, _ctx: &Context) -> Result<()> {
            Ok(())
        }
    }
    impl TaskEventHandler for NoopPlugin {
        fn __handle_task_lifecycle_event(
            &self,
            _event: TaskEvent,
            _payload: TaskEventPayload,
            _ctx: &Context,
        ) -> Result<()> {
            Ok(())
        }
    }
    impl PluginEventHandler for NoopPlugin {
        fn __handle_plugin_event(
            &self,
            _event: PluginEvent,
            _payload: PluginEventPayload,
            _ctx: &Context,
        ) -> Result<()> {
            Ok(())
        }
    }
    impl SampleEventHandler for NoopPlugin {
        fn __handle_sample_event(
            &self,
            _event: SampleEvent,
            _payload: SampleEventPayload,
            _ctx: &Context,
        ) -> Result<()> {
            Ok(())
        }
    }

    fn test_bridge(work_dir: PathBuf) -> GuestPluginBridge<NoopPlugin> {
        GuestPluginBridge {
            plugin: Arc::new(NoopPlugin),
            shutdown_tx: std::sync::Mutex::new(None),
            work_dir,
        }
    }

    // -- normalize_path tests --

    #[test]
    fn normalize_path_resolves_dot_components() {
        let result = normalize_path(&PathBuf::from("/a/b/./c"));
        assert_eq!(result, PathBuf::from("/a/b/c"));
    }

    #[test]
    fn normalize_path_resolves_dotdot_components() {
        let result = normalize_path(&PathBuf::from("/a/b/../c"));
        assert_eq!(result, PathBuf::from("/a/c"));
    }

    #[test]
    fn normalize_path_handles_trailing_dotdot() {
        let result = normalize_path(&PathBuf::from("/a/b/.."));
        assert_eq!(result, PathBuf::from("/a"));
    }

    // -- resolve_path tests --

    #[test]
    fn resolve_path_accepts_simple_relative() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_path(dir.path(), "samples/test.exe");
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with(dir.path()));
    }

    #[test]
    fn resolve_path_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_path(dir.path(), "../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("escapes work directory"));
    }

    #[test]
    fn resolve_path_rejects_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_path(dir.path(), "/etc/passwd");
        assert!(result.is_err());
    }

    // -- on_push_file tests --

    #[tokio::test]
    async fn push_file_writes_to_work_dir() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge
            .on_push_file("samples/test.exe", b"MZ\x90\x00".to_vec())
            .await;
        assert!(result.is_ok());

        let written = std::fs::read(dir.path().join("samples/test.exe")).unwrap();
        assert_eq!(written, b"MZ\x90\x00");
    }

    #[tokio::test]
    async fn push_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge.on_push_file("../escape.txt", b"bad".to_vec()).await;
        assert!(result.is_err());
    }

    // -- on_pull_file tests --

    #[tokio::test]
    async fn pull_file_reads_from_work_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("results/output.json");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, b"{\"key\": \"value\"}").unwrap();

        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge.on_pull_file("results/output.json").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"{\"key\": \"value\"}");
    }

    #[tokio::test]
    async fn pull_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge.on_pull_file("../../etc/passwd").await;
        assert!(result.is_err());
    }

    // -- on_execute_command tests --

    #[tokio::test]
    async fn execute_command_captures_output() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge
            .on_execute_command(
                "echo",
                &["hello".to_string()],
                None,
                HashMap::new(),
                None,
                false,
            )
            .await;

        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.exit_code, Some(0));
        assert_eq!(String::from_utf8_lossy(&resp.stdout).trim(), "hello");
    }

    #[tokio::test]
    async fn execute_command_respects_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge
            .on_execute_command(
                "sleep",
                &["10".to_string()],
                None,
                HashMap::new(),
                Some(50),
                false,
            )
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timed out"));
    }

    #[tokio::test]
    async fn execute_command_background_returns_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge
            .on_execute_command(
                "sleep",
                &["60".to_string()],
                None,
                HashMap::new(),
                None,
                true,
            )
            .await;

        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.exit_code, None);
    }
}
