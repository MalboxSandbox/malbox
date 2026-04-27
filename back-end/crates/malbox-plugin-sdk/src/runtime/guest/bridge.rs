//! `GuestPluginBridge` — adapts the transport's `GuestPluginHandler` trait
//! to the SDK's `Plugin` trait.
//!
//! This is the central glue between gRPC RPCs and user code. Each RPC
//! handler delegates to one of:
//! - The plugin (`Plugin` trait method)
//! - A helper module ([`super::files`], [`super::exec`], [`super::stream`])

use crate::context::Context;
use crate::log::LogBus;
use crate::plugin::Plugin;
use crate::stash::ResultStash;
use crate::types::{ExecRequest, ExecutionInfo};
use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::messages::events::Event;
use malbox_plugin_transport::plugin::{
    GrpcEmitter, GuestPluginHandler, LogEntryStream, ResultChunkStream,
};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tonic::Status;
use tracing::error;

use super::collector::AutoCollectSection;
use super::exec::{ExecutionNotifier, default_execute_command, execution_channel};
use super::files::{pull_file, push_file};
use super::stream::{execute_task, log_entry_to_proto};

/// Bridges incoming gRPC RPCs to the user's `Plugin` trait implementation.
pub(super) struct GuestPluginBridge<P: Send + Sync + 'static> {
    pub(super) plugin: Arc<P>,
    pub(super) shutdown_tx: std::sync::Mutex<Option<oneshot::Sender<()>>>,
    pub(super) sample_dir: PathBuf,
    pub(super) artifact_dir: PathBuf,
    pub(super) external_log_dir: PathBuf,
    pub(super) execution_notifiers: std::sync::Mutex<HashMap<i32, ExecutionNotifier>>,
    /// Most recent execution info, so tasks created after ExecuteCommand
    /// can still pick it up via wait_for_execution.
    pub(super) last_execution: std::sync::Mutex<Option<ExecutionInfo>>,
    pub(super) log_bus: Arc<LogBus>,
    pub(super) stash: Arc<ResultStash>,
    pub(super) auto_collect_artifacts: AutoCollectSection,
    pub(super) auto_collect_external_logs: AutoCollectSection,
}

#[tonic::async_trait]
impl<P: Plugin> GuestPluginHandler for GuestPluginBridge<P> {
    async fn on_initialize(
        &self,
        _plugin_id: i32,
        config: HashMap<String, String>,
    ) -> std::result::Result<Vec<String>, String> {
        let plugin = self.plugin.clone();
        let result = tokio::task::spawn_blocking(move || plugin.on_start(config))
            .await
            .map_err(|e| format!("spawn_blocking panicked: {}", e))?;

        match result {
            Ok(()) => Ok(vec![]),
            Err(e) => Err(e.to_string()),
        }
    }

    async fn on_health_check(&self) -> (bool, String) {
        let plugin = self.plugin.clone();
        tokio::task::spawn_blocking(move || {
            let status = plugin.health_check();
            (status.ready, status.reason)
        })
        .await
        .unwrap_or((false, "health check panicked".to_string()))
    }

    async fn on_shutdown(&self, _graceful: bool) {
        let plugin = self.plugin.clone();
        let _ = tokio::task::spawn_blocking(move || {
            if let Err(e) = plugin.on_stop() {
                error!(error = %e, "Plugin on_stop error");
            }
        })
        .await;

        // Signal the server to stop
        if let Ok(mut guard) = self.shutdown_tx.lock()
            && let Some(tx) = guard.take()
        {
            let _ = tx.send(());
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

        // Resolve the sample path relative to the sample directory so plugin
        // handlers can read it via task.sample_bytes().
        let full_sample_path = if Path::new(&sample_path).is_relative() && !sample_path.is_empty() {
            self.sample_dir.join(&sample_path)
        } else {
            PathBuf::from(&sample_path)
        };

        // Create execution channel so on_task can wait for on_execute_command.
        // If ExecuteCommand already ran before this task started, pre-signal
        // the notifier so wait_for_execution returns immediately.
        let (notifier, waiter) = execution_channel();
        if let Some(info) = self.last_execution.lock().unwrap().clone() {
            notifier.notify(info);
        }
        self.execution_notifiers
            .lock()
            .unwrap()
            .insert(task_id, notifier);

        let stash = Arc::clone(&self.stash);
        let artifact_dir = self.artifact_dir.clone();
        let external_log_dir = self.external_log_dir.clone();
        let auto_collect_artifacts = self.auto_collect_artifacts.clone();
        let auto_collect_external_logs = self.auto_collect_external_logs.clone();
        let _ = tokio::task::spawn_blocking(move || {
            execute_task(
                plugin,
                task_id,
                full_sample_path,
                config,
                result_tx,
                waiter,
                stash,
                artifact_dir,
                external_log_dir,
                auto_collect_artifacts,
                auto_collect_external_logs,
            );
        })
        .await;

        // Clean up the notifier for this task.
        self.execution_notifiers.lock().unwrap().remove(&task_id);
    }

    async fn on_event(&self, event: Event) -> std::result::Result<(), String> {
        let plugin = self.plugin.clone();
        let result = tokio::task::spawn_blocking(move || {
            let emitter = GrpcEmitter::noop();
            let ctx = Context::new(&emitter, None, None);
            plugin.on_event(event, &ctx)
        })
        .await
        .map_err(|e| format!("spawn_blocking panicked: {}", e))?;

        result.map_err(|e| e.to_string())
    }

    async fn on_push_file(&self, dest: &str, data: Vec<u8>) -> std::result::Result<(), String> {
        push_file(&self.sample_dir, dest, data).await
    }

    async fn on_pull_file(&self, source: &str) -> std::result::Result<Vec<u8>, String> {
        pull_file(&self.artifact_dir, source).await
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
        // Check if the plugin overrides command execution
        let plugin = self.plugin.clone();
        let cmd_str = command.to_string();
        let args_clone = args.to_vec();
        let cwd_clone = cwd.map(|s| s.to_string());
        let env_clone = env.clone();
        let timeout = timeout_ms.map(Duration::from_millis);

        let plugin_result = tokio::task::spawn_blocking(move || {
            let request = ExecRequest::new(
                cmd_str, args_clone, cwd_clone, env_clone, timeout, background,
            );
            plugin.on_execute_command(&request)
        })
        .await
        .map_err(|e| format!("spawn_blocking panicked: {}", e))?;

        let response = match plugin_result {
            Some(result) => proto::ExecResponse {
                exit_code: result.exit_code,
                stdout: result.stdout.into_bytes(),
                stderr: result.stderr.into_bytes(),
                pid: result.pid,
            },
            None => {
                default_execute_command(
                    &self.sample_dir,
                    command,
                    args,
                    cwd,
                    env,
                    timeout_ms,
                    background,
                )
                .await?
            }
        };

        // Store execution info and signal all pending notifiers.
        // Storing it ensures tasks created *after* this call can still
        // retrieve the info via wait_for_execution.
        let exec_info = ExecutionInfo::new(response.pid, command.to_string(), args.to_vec());
        *self.last_execution.lock().unwrap() = Some(exec_info.clone());
        let notifiers: Vec<_> = self
            .execution_notifiers
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();
        for notifier in notifiers {
            notifier.notify(exec_info.clone());
        }

        Ok(response)
    }

    async fn on_stream_logs(&self, include_buffered: bool) -> LogEntryStream {
        let log_bus = Arc::clone(&self.log_bus);

        let stream = async_stream::stream! {
            if include_buffered {
                for entry in log_bus.drain_atomic() {
                    yield Ok(log_entry_to_proto(entry));
                }
            }

            loop {
                // Block until entries are available, then drain everything
                // atomically. recv_atomic returns an empty vec only when
                // the bus is closed.
                let entries = log_bus.recv_atomic().await;
                if entries.is_empty() {
                    break;
                }
                for entry in entries {
                    yield Ok(log_entry_to_proto(entry));
                }
            }
        };

        Box::pin(stream)
    }

    async fn on_pull_result(
        &self,
        handle: String,
    ) -> std::result::Result<ResultChunkStream, String> {
        // Take the entry out of the stash so the handle can't be reused.
        let entry = self
            .stash
            .take(&handle)
            .ok_or_else(|| format!("stash handle not found: {handle}"))?;

        // Verify the file is openable before returning a stream. Callers
        // expect pre-stream errors as Err; failures that happen mid-stream
        // yield Err(Status::...) inside the stream.
        let file = tokio::fs::File::open(&entry.path)
            .await
            .map_err(|e| format!("failed to open stashed result: {e}"))?;

        let stash = Arc::clone(&self.stash);

        let stream = async_stream::stream! {
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
                    // Clean EOF — emit empty final chunk so the client
                    // always sees is_last even if the last full chunk was
                    // size-aligned with the file length.
                    yield Ok(proto::ResultChunk {
                        data: vec![],
                        index,
                        is_last: true,
                    });
                    break;
                }
                yield Ok(proto::ResultChunk {
                    data: buf[..n].to_vec(),
                    index,
                    is_last: false,
                });
                index += 1;
            }

            stash.cleanup_after_pull(&entry);
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal plugin impl for testing the bridge.
    struct NoopPlugin;
    impl Plugin for NoopPlugin {}

    fn test_bridge(base_dir: PathBuf) -> GuestPluginBridge<NoopPlugin> {
        let sample_dir = base_dir.join("samples");
        let artifact_dir = base_dir.join("artifacts");
        let external_log_dir = base_dir.join("ext-logs");
        let stash_dir = base_dir.join("_stash");
        std::fs::create_dir_all(&sample_dir).unwrap();
        std::fs::create_dir_all(&artifact_dir).unwrap();
        std::fs::create_dir_all(&external_log_dir).unwrap();
        let stash = Arc::new(
            crate::stash::ResultStash::new(stash_dir, Default::default()).expect("stash init"),
        );
        let disabled_section = AutoCollectSection {
            enabled: false,
            include: vec![],
            exclude: vec![],
            max_file_size: 0,
        };
        GuestPluginBridge {
            plugin: Arc::new(NoopPlugin),
            shutdown_tx: std::sync::Mutex::new(None),
            sample_dir,
            artifact_dir,
            external_log_dir,
            execution_notifiers: std::sync::Mutex::new(HashMap::new()),
            last_execution: std::sync::Mutex::new(None),
            log_bus: Arc::new(LogBus::new(64)),
            stash,
            auto_collect_artifacts: disabled_section.clone(),
            auto_collect_external_logs: disabled_section,
        }
    }

    #[tokio::test]
    async fn bridge_push_file_writes_to_sample_dir() {
        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge
            .on_push_file("test.exe", b"MZ\x90\x00".to_vec())
            .await;
        assert!(result.is_ok());

        let written = std::fs::read(dir.path().join("samples/test.exe")).unwrap();
        assert_eq!(written, b"MZ\x90\x00");
    }

    #[tokio::test]
    async fn bridge_execute_command_uses_default_executor() {
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
    async fn bridge_pull_result_streams_stashed_bytes() {
        use futures::StreamExt;
        use malbox_plugin_transport::plugin::GuestPluginHandler;

        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        // Pre-insert a stash entry.
        let handle = bridge
            .stash
            .insert_bytes(
                1,
                "r".into(),
                crate::stash::StashFormat::Bytes,
                vec![42u8; 200 * 1024], // 200 KB → multiple chunks
            )
            .unwrap();

        let mut stream = bridge
            .on_pull_result(handle.clone())
            .await
            .expect("pull should succeed");

        let mut collected = Vec::new();
        let mut seen_last = false;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.expect("no error expected");
            if chunk.is_last {
                seen_last = true;
                collected.extend_from_slice(&chunk.data);
                break;
            }
            collected.extend_from_slice(&chunk.data);
        }
        assert!(seen_last, "must receive a chunk with is_last=true");
        assert_eq!(collected.len(), 200 * 1024);
        assert!(collected.iter().all(|&b| b == 42));

        // Entry must be gone.
        assert!(bridge.stash.take(&handle).is_none());
    }

    #[tokio::test]
    async fn bridge_pull_result_missing_handle_errors() {
        use malbox_plugin_transport::plugin::GuestPluginHandler;

        let dir = tempfile::tempdir().unwrap();
        let bridge = test_bridge(dir.path().to_path_buf());

        let result = bridge.on_pull_result("nonexistent".to_string()).await;
        assert!(result.is_err(), "unknown handle should error");
    }
}
