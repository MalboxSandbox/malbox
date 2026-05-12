//! Result pushing during task execution.
//!
//! [`ResultSink`] is the plugin-facing API for sending results back to
//! the daemon. Small results travel inline on the control stream; large
//! ones are stashed to disk and replaced with a reference the daemon pulls
//! separately (guest plugins only).

use crate::error::{Result, SdkError};
use crate::result::PluginResult;
use std::path::{Path, PathBuf};

use super::message::{ResultFormat, ResultKind, TaskResultMessage};
use super::{ContextInner, lock_or_err};

#[cfg(feature = "guest")]
use crate::stash::StashFormat;

/// Handle for pushing results back to the daemon during task execution.
///
/// Obtained from [`Context::results()`](super::Context::results). Only
/// usable inside an `on_task` handler; calling [`push`](ResultSink::push)
/// outside of one returns an error.
pub struct ResultSink<'a> {
    pub(super) inner: &'a ContextInner,
}

impl ResultSink<'_> {
    /// Push a single result back to the daemon.
    ///
    /// Small results (below the stash threshold) travel inline on the
    /// control stream. Large byte/JSON payloads are written to the disk
    /// stash and replaced with a reference marker that the daemon pulls
    /// via the `PullResult` RPC (guest plugins only).
    ///
    /// # Panics
    ///
    /// Uses `blocking_send` internally. Must be called from a blocking
    /// context (e.g. `spawn_blocking`), **not** from within an async task.
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
            let guard = lock_or_err(&self.inner.result_tx)?;
            guard
                .as_ref()
                .ok_or(SdkError::InvalidContext("push called outside active task"))?
                .clone()
        };

        let msg = self.classify_and_build(result)?;

        tx.blocking_send(msg)
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

    /// Push a batch of results. Results are sent one at a time in order;
    /// if one fails, the remaining results are not sent and the error is
    /// returned. Results sent before the failure are not rolled back.
    pub fn push_all(&self, results: Vec<PluginResult>) -> Result<()> {
        for result in results {
            self.push(result)?;
        }
        Ok(())
    }

    #[cfg(feature = "guest")]
    fn classify_and_build(&self, result: PluginResult) -> Result<TaskResultMessage> {
        let threshold = self
            .inner
            .stash
            .as_ref()
            .map(|s| s.config().threshold_bytes)
            .unwrap_or(usize::MAX);

        match classify_result(result)? {
            Classified::Inline { name, data, format } if data.len() < threshold => {
                Ok(TaskResultMessage {
                    task_id: self.inner.task_id,
                    result_name: name,
                    data,
                    format,
                    is_final: false,
                    kind: ResultKind::Result,
                    stash_handle: String::new(),
                    stash_format: ResultFormat::Unspecified,
                    stash_size: 0,
                })
            }
            Classified::Inline { name, data, format } => {
                let stash = self.inner.stash.as_ref().ok_or(SdkError::InvalidContext(
                    "push: no stash configured for large payloads",
                ))?;
                let stash_format = result_format_to_stash(format);
                let size_bytes = data.len() as u64;
                let handle =
                    stash.insert_bytes(self.inner.task_id, name.clone(), stash_format, data)?;
                Ok(build_ref_message(
                    self.inner.task_id,
                    handle,
                    name,
                    format,
                    size_bytes,
                ))
            }
            Classified::File { name, path, format } => {
                let stash = self.inner.stash.as_ref().ok_or(SdkError::InvalidContext(
                    "push: no stash configured for file results",
                ))?;
                if let Ok(canonical) = std::fs::canonicalize(&path)
                    && let Ok(mut guard) = self.inner.claimed_paths.lock()
                {
                    guard.insert(canonical);
                }
                let stash_format = result_format_to_stash(format);
                let handle =
                    stash.insert_file(self.inner.task_id, name.clone(), stash_format, path)?;
                let size_bytes = stash.peek_size(&handle).unwrap_or(0);
                Ok(build_ref_message(
                    self.inner.task_id,
                    handle,
                    name,
                    format,
                    size_bytes,
                ))
            }
        }
    }

    #[cfg(not(feature = "guest"))]
    fn classify_and_build(&self, result: PluginResult) -> Result<TaskResultMessage> {
        let classified = classify_result(result)?;
        match classified {
            Classified::Inline { name, data, format } => Ok(TaskResultMessage {
                task_id: self.inner.task_id,
                result_name: name,
                data,
                format,
                is_final: false,
                kind: ResultKind::Result,
                stash_handle: String::new(),
                stash_format: ResultFormat::Unspecified,
                stash_size: 0,
            }),
            Classified::File { .. } => Err(SdkError::InvalidContext(
                "push: file results require guest feature (stash not available)",
            )),
        }
    }
}

/// Result of classifying a `PluginResult` for transport.
#[allow(dead_code)]
pub(crate) enum Classified {
    Inline {
        name: String,
        data: Vec<u8>,
        format: ResultFormat,
    },
    File {
        name: String,
        path: PathBuf,
        format: ResultFormat,
    },
}

/// Classify a `PluginResult` into inline bytes vs a file path.
pub(crate) fn classify_result(result: PluginResult) -> Result<Classified> {
    match result {
        PluginResult::Json { name, data } => Ok(Classified::Inline {
            name,
            data,
            format: ResultFormat::Json,
        }),
        PluginResult::Bytes { name, data } => Ok(Classified::Inline {
            name,
            data,
            format: ResultFormat::Bytes,
        }),
        PluginResult::File { name, path } => Ok(Classified::File {
            name,
            path,
            format: ResultFormat::Bytes,
        }),
    }
}

#[cfg(feature = "guest")]
fn result_format_to_stash(format: ResultFormat) -> StashFormat {
    match format {
        ResultFormat::Json => StashFormat::Json,
        _ => StashFormat::Bytes,
    }
}

#[cfg(feature = "guest")]
fn build_ref_message(
    task_id: i32,
    handle: String,
    _result_name: String,
    format: ResultFormat,
    size_bytes: u64,
) -> TaskResultMessage {
    TaskResultMessage {
        task_id,
        result_name: String::new(),
        data: Vec::new(),
        format: ResultFormat::Unspecified,
        is_final: false,
        kind: ResultKind::ResultRef,
        stash_handle: handle,
        stash_format: format,
        stash_size: size_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use malbox_plugin_transport::traits::TransportEmitter;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn noop_emitter() -> Arc<dyn TransportEmitter + Send + Sync> {
        Arc::new(())
    }

    #[test]
    fn push_outside_active_task_returns_invalid_context() {
        let ctx = Context::new(
            0,
            PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            None,
            #[cfg(feature = "guest")]
            None,
        );
        let result = ctx.results().push(PluginResult::bytes("x", vec![1, 2]));
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
            #[cfg(feature = "guest")]
            None,
        );

        ctx.results()
            .push(PluginResult::bytes("r1", vec![1, 2, 3]))
            .expect("push should succeed");

        let msg = rx.try_recv().expect("channel should have one item");
        assert_eq!(msg.task_id, 42);
        assert_eq!(msg.result_name, "r1");
        assert_eq!(msg.data, vec![1, 2, 3]);
        assert!(!msg.is_final);
    }

    #[cfg(feature = "guest")]
    #[test]
    fn push_large_bytes_goes_to_stash_and_emits_ref() {
        use crate::stash::{ResultStash, StashConfig};

        let tmp = tempfile::tempdir().unwrap();
        let stash = Arc::new(
            ResultStash::new(
                tmp.path().join("_stash"),
                StashConfig {
                    threshold_bytes: 10,
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
            .push(PluginResult::bytes("big", vec![0u8; 64]))
            .expect("push should succeed");

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.task_id, 7);
        assert_eq!(msg.kind, ResultKind::ResultRef);
        assert!(
            msg.result_name.is_empty(),
            "result_name lives in stash metadata, not the ref message"
        );
        assert!(!msg.stash_handle.is_empty());
        assert_eq!(msg.stash_size, 64);
    }

    #[cfg(feature = "guest")]
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
            .push(PluginResult::bytes("small", vec![1, 2, 3]))
            .expect("push should succeed");

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.kind, ResultKind::Result);
        assert_eq!(msg.result_name, "small");
        assert_eq!(msg.data, vec![1, 2, 3]);
    }

    #[cfg(feature = "guest")]
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
                    threshold_bytes: 1024 * 1024,
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
            .push(PluginResult::file("cap", plugin_file.clone()))
            .expect("push should succeed");

        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.kind, ResultKind::ResultRef);
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
            #[cfg(feature = "guest")]
            None,
        );

        assert!(
            ctx.results()
                .push(PluginResult::bytes("ok", vec![1]))
                .is_ok()
        );

        ctx.close_result_channel();

        let result = ctx.results().push(PluginResult::bytes("fail", vec![2]));
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
            #[cfg(feature = "guest")]
            None,
        );

        ctx.results().push_json("j", b"{}").unwrap();
        ctx.results().push_bytes("b", &[1, 2]).unwrap();

        let msg1 = rx.try_recv().unwrap();
        assert_eq!(msg1.result_name, "j");
        assert_eq!(msg1.format, ResultFormat::Json);

        let msg2 = rx.try_recv().unwrap();
        assert_eq!(msg2.result_name, "b");
        assert_eq!(msg2.format, ResultFormat::Bytes);
    }
}
