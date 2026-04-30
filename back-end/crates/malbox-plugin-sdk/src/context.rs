//! Plugin execution context for communicating with the daemon at runtime.

use crate::error::{Result, SdkError};
use crate::stash::{ResultStash, StashFormat};
use crate::types::PluginResult;
use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::messages::events::Event;
use malbox_plugin_transport::traits::TransportEmitter;
use prost::Message;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

/// Sender half of the result streaming channel.
///
/// Used by `Context` to push `TaskResult` messages back to the daemon during
/// task execution. The channel is created by the guest runtime for each
/// `ExecuteTask` RPC and is `None` during lifecycle callbacks.
pub type ResultSender =
    tokio::sync::mpsc::Sender<std::result::Result<proto::TaskResult, tonic::Status>>;

struct ContextInner {
    task_id: i32,
    sample_path: PathBuf,
    config: HashMap<String, String>,
    emitter: Arc<dyn TransportEmitter + Send + Sync>,
    result_tx: Mutex<Option<ResultSender>>,
    stash: Option<Arc<ResultStash>>,
    claimed_paths: Mutex<HashSet<PathBuf>>,
}

/// Runtime context for the current task. Clone to share with background threads.
#[derive(Clone)]
pub struct Context {
    inner: Arc<ContextInner>,
}

impl Context {
    /// Create a new context with task data, a transport emitter, and an
    /// optional result channel.
    ///
    /// Used by the runtime; downstream test code should use
    /// [`Context::test_new`](crate::testkit) under the `testkit` feature.
    pub(crate) fn new(
        task_id: i32,
        sample_path: PathBuf,
        config: HashMap<String, String>,
        emitter: Arc<dyn TransportEmitter + Send + Sync>,
        result_tx: Option<ResultSender>,
        stash: Option<Arc<ResultStash>>,
    ) -> Self {
        Self {
            inner: Arc::new(ContextInner {
                task_id,
                sample_path,
                config,
                emitter,
                result_tx: Mutex::new(result_tx),
                stash,
                claimed_paths: Mutex::new(HashSet::new()),
            }),
        }
    }

    /// Access task information (id, sample path, config).
    pub fn task(&self) -> TaskInfo<'_> {
        TaskInfo { inner: &self.inner }
    }

    /// Access result-pushing methods.
    pub fn results(&self) -> ResultSink<'_> {
        ResultSink { inner: &self.inner }
    }

    /// Report execution progress (0.0 to 1.0) with a status message.
    ///
    /// Progress updates are streamed to the daemon in real-time and can be
    /// shown in the UI. The `progress` value is clamped to `[0.0, 1.0]`.
    ///
    /// If a result channel is available, a PROGRESS-kind `TaskResult` is sent
    /// over it. Otherwise, just logs the progress.
    pub fn progress(&self, pct: f64, message: &str) -> Result<()> {
        let clamped = pct.clamp(0.0, 1.0);
        info!(kind = "progress", progress = clamped, %message, "Plugin progress");

        let tx = {
            let guard = self.inner.result_tx.lock().unwrap();
            guard.clone()
        };

        if let Some(tx) = tx {
            // Encode progress as JSON in the data field
            let progress_data = serde_json::to_vec(&serde_json::json!({
                "progress": clamped,
                "message": message,
            }))?;

            let task_result = proto::TaskResult {
                task_id: self.inner.task_id,
                result_name: String::new(),
                data: progress_data,
                format: proto::ResultFormat::Json.into(),
                is_final: false,
                kind: proto::ResultKind::Progress.into(),
            };

            tx.blocking_send(Ok(task_result))
                .map_err(|e| SdkError::Channel(format!("progress: {e}")))?;
        }

        Ok(())
    }

    /// Emit an event back to the daemon.
    ///
    /// This is an escape hatch for advanced use cases. Prefer the typed
    /// helper methods when possible.
    pub fn emit_event(&self, event: Event) -> Result<()> {
        self.inner.emitter.emit(event).map_err(SdkError::Transport)
    }

    /// Log a warning that will be attached to the task report.
    pub fn warn(&self, message: &str) -> Result<()> {
        warn!(kind = "warning", %message, "Plugin warning");
        Ok(())
    }

    /// Mark a file as already handled, preventing artifact auto-collection
    /// from sending it again.
    ///
    /// Use this when a plugin reads a file from the artifacts directory,
    /// processes it, and sends derived data as Json/Bytes instead of the
    /// raw file.
    pub fn mark_collected(&self, path: impl AsRef<Path>) {
        if let Ok(canonical) = std::fs::canonicalize(path.as_ref()) {
            self.inner.claimed_paths.lock().unwrap().insert(canonical);
        }
    }

    /// Close the result channel, preventing further pushes.
    ///
    /// Called by the runtime after `on_stop` to signal end-of-task.
    #[allow(dead_code)]
    pub(crate) fn close_result_channel(&self) {
        let mut guard = self.inner.result_tx.lock().unwrap();
        *guard = None;
    }

    /// Return a snapshot of all claimed file paths.
    ///
    /// Used internally by auto-collection to determine which files
    /// have already been explicitly sent or marked.
    pub(crate) fn claimed_paths(&self) -> HashSet<PathBuf> {
        self.inner.claimed_paths.lock().unwrap().clone()
    }
}

/// Read-only view of task information (id, sample path, config).
pub struct TaskInfo<'a> {
    inner: &'a ContextInner,
}

impl TaskInfo<'_> {
    /// Return the task's numeric identifier.
    pub fn id(&self) -> i32 {
        self.inner.task_id
    }

    /// Get the path to the sample file.
    pub fn sample_path(&self) -> &Path {
        &self.inner.sample_path
    }

    /// Read the entire sample file into memory.
    pub fn sample_bytes(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.inner.sample_path).map_err(SdkError::Io)
    }

    /// Return the task's configuration map.
    pub fn config(&self) -> &HashMap<String, String> {
        &self.inner.config
    }

    /// Look up a single configuration value by key.
    pub fn config_value(&self, key: &str) -> Option<&str> {
        self.inner.config.get(key).map(|s| s.as_str())
    }
}

/// Push results back to the daemon during task execution.
pub struct ResultSink<'a> {
    inner: &'a ContextInner,
}

impl ResultSink<'_> {
    /// Push a single result back to the daemon.
    ///
    /// Small results (below the stash threshold) travel inline on the
    /// control stream. Large byte/JSON payloads are written to the disk
    /// stash and replaced with a `RESULT_REF` marker that the daemon pulls
    /// via the `PullResult` RPC. `PluginResult::File` variants always go
    /// through the stash without being read into memory.
    ///
    /// # Errors
    ///
    /// - Returns `SdkError::InvalidContext` if called outside an `on_task` handler
    ///   (when the context has no result channel).
    /// - Returns `SdkError::Channel` if the result channel is closed (typically
    ///   because the daemon disconnected).
    /// - Returns `SdkError::Io` if the result is a `File` variant and the file
    ///   cannot be read.
    pub fn push(&self, result: PluginResult) -> Result<()> {
        let tx = {
            let guard = self.inner.result_tx.lock().unwrap();
            guard
                .as_ref()
                .ok_or(SdkError::InvalidContext("push called outside active task"))?
                .clone()
        };

        let threshold = self
            .inner
            .stash
            .as_ref()
            .map(|s| s.config().threshold_bytes)
            .unwrap_or(usize::MAX);

        let task_result = match classify_result(result)? {
            Classified::Inline { name, data, format } if data.len() < threshold => {
                // Fast path: small result, stays on the control stream.
                proto::TaskResult {
                    task_id: self.inner.task_id,
                    result_name: name,
                    data,
                    format: format.into(),
                    is_final: false,
                    kind: proto::ResultKind::Result.into(),
                }
            }
            Classified::Inline { name, data, format } => {
                // Large in-memory payload: offload to stash + emit ref.
                let stash = self.inner.stash.as_ref().ok_or(SdkError::InvalidContext(
                    "push: no stash configured for large payloads",
                ))?;
                let stash_format = proto_format_to_stash(format);
                let size_bytes = data.len() as u64;
                let handle =
                    stash.insert_bytes(self.inner.task_id, name.clone(), stash_format, data)?;
                build_ref_message(self.inner.task_id, handle, name, format, size_bytes)
            }
            Classified::File { name, path, format } => {
                // Plugin-owned file: always stash as ref, no copy, no delete.
                let stash = self.inner.stash.as_ref().ok_or(SdkError::InvalidContext(
                    "push: no stash configured for file results",
                ))?;
                if let Ok(canonical) = std::fs::canonicalize(&path) {
                    self.inner.claimed_paths.lock().unwrap().insert(canonical);
                }
                let stash_format = proto_format_to_stash(format);
                let handle =
                    stash.insert_file(self.inner.task_id, name.clone(), stash_format, path)?;
                let size_bytes = stash.peek_size(&handle).unwrap_or(0);
                build_ref_message(self.inner.task_id, handle, name, format, size_bytes)
            }
        };

        tx.blocking_send(Ok(task_result))
            .map_err(|e| SdkError::Channel(format!("push: {e}")))?;
        Ok(())
    }

    /// Push a JSON result (raw bytes that are already JSON-encoded).
    pub fn push_json(&self, name: &str, data: &[u8]) -> Result<()> {
        self.push(PluginResult::Json {
            name: name.into(),
            data: data.to_vec(),
        })
    }

    /// Push a raw byte result.
    pub fn push_bytes(&self, name: &str, data: &[u8]) -> Result<()> {
        self.push(PluginResult::Bytes {
            name: name.into(),
            data: data.to_vec(),
        })
    }

    /// Push a file result by path.
    pub fn push_file(&self, name: &str, path: impl AsRef<Path>) -> Result<()> {
        self.push(PluginResult::File {
            name: name.into(),
            path: path.as_ref().to_path_buf(),
        })
    }

    /// Push a batch of results.
    pub fn flush(&self, results: Vec<PluginResult>) -> Result<()> {
        for result in results {
            self.push(result)?;
        }
        Ok(())
    }
}

/// Result of classifying a `PluginResult` for transport.
pub(crate) enum Classified {
    /// Payload is in memory and can be sent inline (if small enough).
    Inline {
        name: String,
        data: Vec<u8>,
        format: proto::ResultFormat,
    },
    /// Payload is a plugin-owned file on disk -- always goes via stash.
    File {
        name: String,
        path: std::path::PathBuf,
        format: proto::ResultFormat,
    },
}

/// Classify a `PluginResult` into inline bytes vs a file path.
pub(crate) fn classify_result(result: PluginResult) -> Result<Classified> {
    match result {
        PluginResult::Json { name, data } => Ok(Classified::Inline {
            name,
            data,
            format: proto::ResultFormat::Json,
        }),
        PluginResult::Bytes { name, data } => Ok(Classified::Inline {
            name,
            data,
            format: proto::ResultFormat::Bytes,
        }),
        PluginResult::File { name, path } => Ok(Classified::File {
            name,
            path,
            format: proto::ResultFormat::Bytes,
        }),
    }
}

fn proto_format_to_stash(format: proto::ResultFormat) -> StashFormat {
    match format {
        proto::ResultFormat::Json => StashFormat::Json,
        _ => StashFormat::Bytes,
    }
}

fn build_ref_message(
    task_id: i32,
    handle: String,
    result_name: String,
    format: proto::ResultFormat,
    size_bytes: u64,
) -> proto::TaskResult {
    let ref_msg = proto::ResultRef {
        handle,
        result_name,
        format: format.into(),
        size_bytes,
    };
    proto::TaskResult {
        task_id,
        result_name: String::new(),
        data: ref_msg.encode_to_vec(),
        format: proto::ResultFormat::Unspecified.into(),
        is_final: false,
        kind: proto::ResultKind::ResultRef.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_emitter() -> Arc<dyn TransportEmitter + Send + Sync> {
        Arc::new(())
    }

    #[test]
    fn context_is_clone() {
        let ctx = Context::test_new(noop_emitter());
        let ctx2 = ctx.clone();
        assert_eq!(ctx.task().id(), ctx2.task().id());
    }

    #[test]
    fn context_progress_clamps_values() {
        let ctx = Context::test_new(noop_emitter());

        // Should not panic for out-of-range values
        assert!(ctx.progress(-0.5, "negative").is_ok());
        assert!(ctx.progress(1.5, "over").is_ok());
        assert!(ctx.progress(0.5, "normal").is_ok());
    }

    #[test]
    fn context_warn_succeeds() {
        let ctx = Context::test_new(noop_emitter());
        assert!(ctx.warn("test warning").is_ok());
    }

    #[test]
    fn context_emit_event_succeeds_with_noop() {
        let ctx = Context::test_new(noop_emitter());
        let result = ctx.emit_event(Event::PluginStarted { plugin_id: 1 });
        assert!(result.is_ok());
    }

    #[test]
    fn task_info_accessors_work() {
        let mut config = HashMap::new();
        config.insert("k".to_string(), "v".to_string());
        let ctx = Context::new(
            7,
            PathBuf::from("/tmp/s.bin"),
            config,
            noop_emitter(),
            None,
            None,
        );
        assert_eq!(ctx.task().id(), 7);
        assert_eq!(ctx.task().sample_path(), Path::new("/tmp/s.bin"));
        assert_eq!(ctx.task().config().get("k"), Some(&"v".to_string()));
        assert_eq!(ctx.task().config_value("k"), Some("v"));
        assert_eq!(ctx.task().config_value("missing"), None);
    }

    #[test]
    fn push_outside_active_task_returns_invalid_context() {
        let ctx = Context::new(
            0,
            PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            None,
            None,
        );
        let result = ctx
            .results()
            .push(crate::types::PluginResult::bytes("x", vec![1, 2]));
        assert!(matches!(
            result,
            Err(SdkError::InvalidContext("push called outside active task"))
        ));
    }

    #[test]
    fn push_inside_active_task_sends_to_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let ctx = Context::new(
            42,
            PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            Some(tx),
            None,
        );

        ctx.results()
            .push(crate::types::PluginResult::bytes("r1", vec![1, 2, 3]))
            .expect("push should succeed");

        let received = rx.try_recv().expect("channel should have one item");
        let task_result = received.expect("should be Ok");
        assert_eq!(task_result.task_id, 42);
        assert_eq!(task_result.result_name, "r1");
        assert_eq!(task_result.data, vec![1, 2, 3]);
        assert!(!task_result.is_final);
    }

    #[test]
    fn push_large_bytes_goes_to_stash_and_emits_ref() {
        use crate::stash::{ResultStash, StashConfig};

        let tmp = tempfile::tempdir().unwrap();
        let stash = Arc::new(
            ResultStash::new(
                tmp.path().join("_stash"),
                StashConfig {
                    threshold_bytes: 10, // anything over 10 bytes is large
                    ttl: std::time::Duration::from_secs(60),
                },
            )
            .unwrap(),
        );

        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let ctx = Context::new(
            7,
            PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            Some(tx),
            Some(Arc::clone(&stash)),
        );

        ctx.results()
            .push(crate::types::PluginResult::bytes("big", vec![0u8; 64]))
            .expect("push should succeed");

        let msg = rx.try_recv().unwrap().unwrap();
        assert_eq!(msg.task_id, 7);
        assert_eq!(msg.kind, proto::ResultKind::ResultRef as i32);
        assert!(
            msg.result_name.is_empty(),
            "result_name lives in ResultRef, not TaskResult"
        );

        // Decode the ResultRef from data
        use prost::Message;
        let ref_msg = proto::ResultRef::decode(msg.data.as_slice()).unwrap();
        assert_eq!(ref_msg.result_name, "big");
        assert_eq!(ref_msg.size_bytes, 64);
        assert!(!ref_msg.handle.is_empty());
    }

    #[test]
    fn push_small_bytes_stays_inline() {
        use crate::stash::{ResultStash, StashConfig};

        let tmp = tempfile::tempdir().unwrap();
        let stash = Arc::new(
            ResultStash::new(
                tmp.path().join("_stash"),
                StashConfig {
                    threshold_bytes: 1024,
                    ttl: std::time::Duration::from_secs(60),
                },
            )
            .unwrap(),
        );

        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let ctx = Context::new(
            8,
            PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            Some(tx),
            Some(Arc::clone(&stash)),
        );

        ctx.results()
            .push(crate::types::PluginResult::bytes("small", vec![1, 2, 3]))
            .expect("push should succeed");

        let msg = rx.try_recv().unwrap().unwrap();
        assert_eq!(msg.kind, proto::ResultKind::Result as i32);
        assert_eq!(msg.result_name, "small");
        assert_eq!(msg.data, vec![1, 2, 3]);
    }

    #[test]
    fn push_file_variant_always_goes_to_stash() {
        use crate::stash::{ResultStash, StashConfig};

        let tmp = tempfile::tempdir().unwrap();
        let plugin_file = tmp.path().join("artifact.bin");
        std::fs::write(&plugin_file, b"plugin-owned").unwrap();

        let stash = Arc::new(
            ResultStash::new(
                tmp.path().join("_stash"),
                StashConfig {
                    threshold_bytes: 1024 * 1024, // high threshold
                    ttl: std::time::Duration::from_secs(60),
                },
            )
            .unwrap(),
        );

        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let ctx = Context::new(
            9,
            PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            Some(tx),
            Some(Arc::clone(&stash)),
        );

        ctx.results()
            .push(crate::types::PluginResult::file("cap", plugin_file.clone()))
            .expect("push should succeed");

        let msg = rx.try_recv().unwrap().unwrap();
        assert_eq!(msg.kind, proto::ResultKind::ResultRef as i32);
        // File must not have been touched.
        assert!(plugin_file.exists());
    }

    #[test]
    fn close_result_channel_makes_push_fail() {
        let (tx, _rx) = tokio::sync::mpsc::channel(4);
        let ctx = Context::new(
            0,
            PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            Some(tx),
            None,
        );

        // Push should work before close
        assert!(
            ctx.results()
                .push(crate::types::PluginResult::bytes("ok", vec![1]))
                .is_ok()
        );

        // Close the channel
        ctx.close_result_channel();

        // Push should fail after close
        let result = ctx
            .results()
            .push(crate::types::PluginResult::bytes("fail", vec![2]));
        assert!(matches!(
            result,
            Err(SdkError::InvalidContext("push called outside active task"))
        ));
    }

    #[test]
    fn result_sink_convenience_methods() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let ctx = Context::new(
            1,
            PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            Some(tx),
            None,
        );

        ctx.results().push_json("j", b"{}").unwrap();
        ctx.results().push_bytes("b", &[1, 2]).unwrap();

        let msg1 = rx.try_recv().unwrap().unwrap();
        assert_eq!(msg1.result_name, "j");
        assert_eq!(msg1.format, proto::ResultFormat::Json as i32);

        let msg2 = rx.try_recv().unwrap().unwrap();
        assert_eq!(msg2.result_name, "b");
        assert_eq!(msg2.format, proto::ResultFormat::Bytes as i32);
    }
}
