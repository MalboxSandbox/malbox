//! Plugin execution context for communicating with the daemon at runtime.

use crate::error::{Result, SdkError};
use crate::stash::{ResultStash, StashFormat};
use crate::types::PluginResult;
use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::messages::events::Event;
use malbox_plugin_transport::traits::TransportEmitter;
use prost::Message;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{info, warn};

/// Sender half of the result streaming channel.
///
/// Used by `Context` to push `TaskResult` messages back to the daemon during
/// task execution. The channel is created by the guest runtime for each
/// `ExecuteTask` RPC and is `None` during lifecycle callbacks.
pub type ResultSender =
    tokio::sync::mpsc::Sender<std::result::Result<proto::TaskResult, tonic::Status>>;

/// Runtime context available to plugin handler methods.
///
/// Provides methods for emitting progress updates, events, and warnings
/// back to the daemon during task execution.
pub struct Context<'a> {
    emitter: &'a dyn TransportEmitter,
    /// Optional channel for streaming results back to the daemon.
    /// Present during `on_task` handlers, `None` during lifecycle callbacks.
    result_tx: Option<ResultSender>,
    /// Current task ID (needed for result messages). Defaults to 0.
    task_id: i32,
    /// Optional stash for offloading large payloads to disk.
    stash: Option<Arc<ResultStash>>,
    /// Canonical paths of files that have been explicitly sent via
    /// `push_result(File { .. })` or marked via `mark_collected()`.
    /// Used by artifact auto-collection to avoid sending duplicates.
    claimed_paths: std::sync::Mutex<HashSet<PathBuf>>,
}

impl<'a> Context<'a> {
    /// Create a new context wrapping a transport emitter with an optional
    /// result channel.
    ///
    /// Used by the runtime; downstream test code should use
    /// [`Context::test_new`](crate::testkit) under the `testkit` feature.
    pub(crate) fn new(emitter: &'a dyn TransportEmitter, result_tx: Option<ResultSender>) -> Self {
        Self {
            emitter,
            result_tx,
            task_id: 0,
            stash: None,
            claimed_paths: std::sync::Mutex::new(HashSet::new()),
        }
    }

    /// Set the task ID for this context. Returns self for builder-style usage.
    #[must_use = "with_task_id consumes self and returns a new Context; the returned value must be used"]
    pub fn with_task_id(mut self, task_id: i32) -> Self {
        self.task_id = task_id;
        self
    }

    /// Attach a result stash for large-payload offloading. Returns self for
    /// builder-style usage.
    #[must_use = "with_stash consumes self and returns a new Context"]
    pub fn with_stash(mut self, stash: Arc<ResultStash>) -> Self {
        self.stash = Some(stash);
        self
    }

    /// Report execution progress (0.0 to 1.0) with a status message.
    ///
    /// Progress updates are streamed to the daemon in real-time and can be
    /// shown in the UI. The `progress` value is clamped to `[0.0, 1.0]`.
    ///
    /// If a result channel is available, a PROGRESS-kind `TaskResult` is sent
    /// over it. Otherwise, just logs the progress.
    pub fn emit_progress(&self, progress: f64, message: &str) -> Result<()> {
        let clamped = progress.clamp(0.0, 1.0);
        info!(kind = "progress", progress = clamped, %message, "Plugin progress");

        if let Some(ref tx) = self.result_tx {
            // Encode progress as JSON in the data field
            let progress_data = serde_json::to_vec(&serde_json::json!({
                "progress": clamped,
                "message": message,
            }))?;

            let task_result = proto::TaskResult {
                task_id: self.task_id,
                result_name: String::new(),
                data: progress_data,
                format: proto::ResultFormat::Json.into(),
                is_final: false,
                kind: proto::ResultKind::Progress.into(),
            };

            tx.blocking_send(Ok(task_result))
                .map_err(|e| SdkError::Channel(format!("emit_progress: {e}")))?;
        }

        Ok(())
    }

    /// Emit an event back to the daemon.
    ///
    /// This is an escape hatch for advanced use cases. Prefer the typed
    /// helper methods when possible.
    pub fn emit_event(&self, event: Event) -> Result<()> {
        self.emitter.emit(event).map_err(SdkError::Transport)
    }

    /// Log a warning that will be attached to the task report.
    pub fn warn(&self, message: &str) -> Result<()> {
        warn!(kind = "warning", %message, "Plugin warning");
        Ok(())
    }

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
    pub fn push_result(&self, result: PluginResult) -> Result<()> {
        let tx = self.result_tx.as_ref().ok_or(SdkError::InvalidContext(
            "push_result called outside on_task",
        ))?;

        let threshold = self
            .stash
            .as_ref()
            .map(|s| s.config().threshold_bytes)
            .unwrap_or(usize::MAX);

        let task_result = match classify_result(result)? {
            Classified::Inline { name, data, format } if data.len() < threshold => {
                // Fast path: small result, stays on the control stream.
                proto::TaskResult {
                    task_id: self.task_id,
                    result_name: name,
                    data,
                    format: format.into(),
                    is_final: false,
                    kind: proto::ResultKind::Result.into(),
                }
            }
            Classified::Inline { name, data, format } => {
                // Large in-memory payload: offload to stash + emit ref.
                let stash = self.stash.as_ref().ok_or(SdkError::InvalidContext(
                    "push_result: no stash configured for large payloads",
                ))?;
                let stash_format = proto_format_to_stash(format);
                let size_bytes = data.len() as u64;
                let handle = stash.insert_bytes(self.task_id, name.clone(), stash_format, data)?;
                build_ref_message(self.task_id, handle, name, format, size_bytes)
            }
            Classified::File { name, path, format } => {
                // Plugin-owned file: always stash as ref, no copy, no delete.
                let stash = self.stash.as_ref().ok_or(SdkError::InvalidContext(
                    "push_result: no stash configured for file results",
                ))?;
                if let Ok(canonical) = std::fs::canonicalize(&path) {
                    self.claimed_paths.lock().unwrap().insert(canonical);
                }
                let stash_format = proto_format_to_stash(format);
                let handle = stash.insert_file(self.task_id, name.clone(), stash_format, path)?;
                let size_bytes = stash.peek_size(&handle).unwrap_or(0);
                build_ref_message(self.task_id, handle, name, format, size_bytes)
            }
        };

        tx.blocking_send(Ok(task_result))
            .map_err(|e| SdkError::Channel(format!("push_result: {e}")))?;
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
            self.claimed_paths.lock().unwrap().insert(canonical);
        }
    }

    /// Return a snapshot of all claimed file paths.
    ///
    /// Used internally by auto-collection to determine which files
    /// have already been explicitly sent or marked.
    pub(crate) fn claimed_paths(&self) -> HashSet<PathBuf> {
        self.claimed_paths.lock().unwrap().clone()
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
    /// Payload is a plugin-owned file on disk — always goes via stash.
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

    #[test]
    fn context_emit_progress_clamps_values() {
        // Use no-op emitter (unit type implements TransportEmitter)
        let emitter = ();
        let ctx = Context::test_new(&emitter);

        // Should not panic for out-of-range values
        assert!(ctx.emit_progress(-0.5, "negative").is_ok());
        assert!(ctx.emit_progress(1.5, "over").is_ok());
        assert!(ctx.emit_progress(0.5, "normal").is_ok());
    }

    #[test]
    fn context_warn_succeeds() {
        let emitter = ();
        let ctx = Context::test_new(&emitter);
        assert!(ctx.warn("test warning").is_ok());
    }

    #[test]
    fn context_emit_event_succeeds_with_noop() {
        let emitter = ();
        let ctx = Context::test_new(&emitter);
        let result = ctx.emit_event(Event::PluginStarted { plugin_id: 1 });
        assert!(result.is_ok());
    }

    #[test]
    fn push_result_outside_on_task_returns_invalid_context() {
        let emitter = ();
        let ctx = Context::new(&emitter, None);
        let result = ctx.push_result(crate::types::PluginResult::bytes("x", vec![1, 2]));
        assert!(matches!(
            result,
            Err(SdkError::InvalidContext(
                "push_result called outside on_task"
            ))
        ));
    }

    #[test]
    fn push_result_inside_on_task_sends_to_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let emitter = ();
        let ctx = Context::new(&emitter, Some(tx)).with_task_id(42);

        ctx.push_result(crate::types::PluginResult::bytes("r1", vec![1, 2, 3]))
            .expect("push should succeed");

        let received = rx.try_recv().expect("channel should have one item");
        let task_result = received.expect("should be Ok");
        assert_eq!(task_result.task_id, 42);
        assert_eq!(task_result.result_name, "r1");
        assert_eq!(task_result.data, vec![1, 2, 3]);
        assert!(!task_result.is_final);
    }

    #[test]
    fn push_result_large_bytes_goes_to_stash_and_emits_ref() {
        use crate::stash::{ResultStash, StashConfig};
        use std::sync::Arc;

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
        let emitter = ();
        let ctx = Context::new(&emitter, Some(tx))
            .with_task_id(7)
            .with_stash(Arc::clone(&stash));

        ctx.push_result(crate::types::PluginResult::bytes("big", vec![0u8; 64]))
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
    fn push_result_small_bytes_stays_inline() {
        use crate::stash::{ResultStash, StashConfig};
        use std::sync::Arc;

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
        let emitter = ();
        let ctx = Context::new(&emitter, Some(tx))
            .with_task_id(8)
            .with_stash(Arc::clone(&stash));

        ctx.push_result(crate::types::PluginResult::bytes("small", vec![1, 2, 3]))
            .expect("push should succeed");

        let msg = rx.try_recv().unwrap().unwrap();
        assert_eq!(msg.kind, proto::ResultKind::Result as i32);
        assert_eq!(msg.result_name, "small");
        assert_eq!(msg.data, vec![1, 2, 3]);
    }

    #[test]
    fn push_result_file_variant_always_goes_to_stash() {
        use crate::stash::{ResultStash, StashConfig};
        use std::sync::Arc;

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
        let emitter = ();
        let ctx = Context::new(&emitter, Some(tx))
            .with_task_id(9)
            .with_stash(Arc::clone(&stash));

        ctx.push_result(crate::types::PluginResult::file("cap", plugin_file.clone()))
            .expect("push should succeed");

        let msg = rx.try_recv().unwrap().unwrap();
        assert_eq!(msg.kind, proto::ResultKind::ResultRef as i32);
        // File must not have been touched.
        assert!(plugin_file.exists());
    }
}
