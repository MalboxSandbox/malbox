//! Result pushing during task execution.
//!
//! [`ResultSink`] is the plugin-facing API for sending results back to
//! the daemon. Small results travel inline on the control stream; large
//! ones are stashed to disk and replaced with a reference the daemon pulls
//! separately (guest plugins only). Without a stash (host plugins), file
//! results are read and sent inline, up to [`HOST_FILE_RESULT_MAX_BYTES`].

use crate::artifact::{ArtifactMeta, ArtifactOpts, ArtifactRelation, ArtifactSource};
use crate::error::{Result, SdkError};
use crate::result::PluginResult;
use malbox_plugin_transport::job_config;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::message::{ResultFormat, ResultKind, TaskResultMessage};
use super::{ContextInner, ResultSender, lock_or_err};

#[cfg(feature = "guest")]
use crate::stash::StashFormat;

/// Largest file result sent inline when no stash is available.
pub const HOST_FILE_RESULT_MAX_BYTES: u64 = 256 * 1024 * 1024;

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
    /// # Errors
    ///
    /// - Returns `SdkError::InvalidContext` if called outside an `on_task` handler
    ///   (when the context has no result channel).
    /// - Returns `SdkError::Channel` if the result channel is closed (typically
    ///   because the daemon disconnected).
    /// - Returns `SdkError::Io` if the result is a `File` variant and the file
    ///   cannot be read.
    pub fn push(&self, result: PluginResult) -> Result<()> {
        let tx = self.sender()?;
        self.send(&tx, result, None)
    }

    /// Push a file the plugin extracted, dropped, unpacked, dumped or
    /// produced as an artifact of the current one.
    ///
    /// Analysed artifacts (`opts` resolves to `analyze = true`) count
    /// against the job's `malbox.max_artifacts` budget; once it is spent
    /// this returns [`SdkError::ArtifactLimit`] without sending anything.
    /// A push that fails does not consume budget.
    ///
    /// # Errors
    ///
    /// - Returns `SdkError::ArtifactLimit` if the artifact would be analysed
    ///   and the job's artifact budget is spent.
    /// - Returns `SdkError::InvalidContext` if `opts.relation` is the reserved
    ///   `Root` relation, or if called outside an `on_task` handler.
    /// - Returns `SdkError::Io` if `source` is a file that cannot be read.
    /// - Returns `SdkError::Channel` if the result channel is closed.
    pub fn push_artifact(
        &self,
        name: &str,
        source: ArtifactSource,
        opts: ArtifactOpts,
    ) -> Result<()> {
        if opts.relation == ArtifactRelation::Root {
            return Err(SdkError::InvalidContext(
                "artifact relation root is reserved",
            ));
        }
        let meta = opts.resolve();
        let tx = self.sender()?;
        if meta.analyze {
            self.reserve_artifact_slot()?;
        }
        let result = match source {
            ArtifactSource::Bytes(data) => PluginResult::Bytes {
                name: name.into(),
                data,
            },
            ArtifactSource::File(path) => PluginResult::File {
                name: name.into(),
                path,
            },
        };
        let sent = self.send(&tx, result, Some(meta));
        if meta.analyze && sent.is_err() {
            self.inner.analysed_pushes.fetch_sub(1, Ordering::SeqCst);
        }
        sent
    }

    /// Count one analysed artifact against the budget, failing once it is spent.
    fn reserve_artifact_slot(&self) -> Result<()> {
        let limit = self.max_artifacts();
        self.inner
            .analysed_pushes
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| match limit {
                Some(limit) if count >= limit => None,
                _ => Some(count + 1),
            })
            .map(drop)
            .map_err(|_| SdkError::ArtifactLimit {
                limit: limit.unwrap_or_default(),
            })
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

    fn sender(&self) -> Result<ResultSender> {
        let guard = lock_or_err(&self.inner.result_tx)?;
        guard
            .as_ref()
            .cloned()
            .ok_or(SdkError::InvalidContext("push called outside active task"))
    }

    fn send(
        &self,
        tx: &ResultSender,
        result: PluginResult,
        artifact: Option<ArtifactMeta>,
    ) -> Result<()> {
        let mut msg = self.classify_and_build(result)?;
        msg.artifact = artifact;
        tx.send(msg)
            .map_err(|e| SdkError::Channel(format!("push: {e}")))
    }

    fn max_artifacts(&self) -> Option<u64> {
        self.inner
            .config
            .get(job_config::MAX_ARTIFACTS)
            .and_then(|v| v.parse::<u64>().ok())
    }

    fn inline_message(
        &self,
        name: String,
        data: Vec<u8>,
        format: ResultFormat,
    ) -> TaskResultMessage {
        TaskResultMessage {
            task_id: self.inner.task_id,
            result_name: name,
            data,
            format,
            is_final: false,
            kind: ResultKind::Result,
            stash_handle: String::new(),
            stash_format: ResultFormat::Unspecified,
            stash_size: 0,
            artifact: None,
        }
    }

    fn claim(&self, path: &Path) {
        if let Ok(canonical) = std::fs::canonicalize(path)
            && let Ok(mut guard) = self.inner.claimed_paths.lock()
        {
            guard.insert(canonical);
        }
    }

    /// Read a file result into memory for inline transport (no stash).
    fn inline_file(
        &self,
        name: String,
        path: &Path,
        format: ResultFormat,
    ) -> Result<TaskResultMessage> {
        let data = read_file_inline(path)?;
        self.claim(path);
        Ok(self.inline_message(name, data, format))
    }

    #[cfg(feature = "guest")]
    fn classify_and_build(&self, result: PluginResult) -> Result<TaskResultMessage> {
        let Some(stash) = self.inner.stash.as_ref() else {
            return match classify_result(result)? {
                Classified::Inline { name, data, format } => {
                    Ok(self.inline_message(name, data, format))
                }
                Classified::File { name, path, format } => self.inline_file(name, &path, format),
            };
        };
        let threshold = stash.config().threshold_bytes;

        match classify_result(result)? {
            Classified::Inline { name, data, format } if data.len() < threshold => {
                Ok(self.inline_message(name, data, format))
            }
            Classified::Inline { name, data, format } => {
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
                self.claim(&path);
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
        match classify_result(result)? {
            Classified::Inline { name, data, format } => {
                Ok(self.inline_message(name, data, format))
            }
            Classified::File { name, path, format } => self.inline_file(name, &path, format),
        }
    }
}

/// Read a file for inline transport, enforcing [`HOST_FILE_RESULT_MAX_BYTES`].
pub(crate) fn read_file_inline(path: &Path) -> Result<Vec<u8>> {
    let len = std::fs::metadata(path)?.len();
    if len > HOST_FILE_RESULT_MAX_BYTES {
        return Err(SdkError::InvalidContext(
            "file result exceeds 256 MiB host limit",
        ));
    }
    Ok(std::fs::read(path)?)
}

/// Result of classifying a `PluginResult` for transport.
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
    result_name: String,
    format: ResultFormat,
    size_bytes: u64,
) -> TaskResultMessage {
    TaskResultMessage {
        task_id,
        result_name,
        data: Vec::new(),
        format: ResultFormat::Unspecified,
        is_final: false,
        kind: ResultKind::ResultRef,
        stash_handle: handle,
        stash_format: format,
        stash_size: size_bytes,
        artifact: None,
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
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
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

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
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
        assert_eq!(msg.result_name, "big");
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

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
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

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
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
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
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
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
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

    fn artifact_ctx(
        config: HashMap<String, String>,
    ) -> (
        Context,
        tokio::sync::mpsc::UnboundedReceiver<TaskResultMessage>,
    ) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let ctx = Context::new(
            3,
            PathBuf::new(),
            config,
            noop_emitter(),
            Some(tx),
            #[cfg(feature = "guest")]
            None,
        );
        (ctx, rx)
    }

    #[test]
    fn push_artifact_sends_meta() {
        let (ctx, mut rx) = artifact_ctx(HashMap::new());
        ctx.results()
            .push_artifact(
                "x.exe",
                ArtifactSource::Bytes(vec![1, 2]),
                ArtifactOpts::new(ArtifactRelation::Dropped),
            )
            .unwrap();
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.result_name, "x.exe");
        assert_eq!(msg.data, vec![1, 2]);
        assert_eq!(
            msg.artifact,
            Some(ArtifactMeta {
                relation: ArtifactRelation::Dropped,
                analyze: true,
                allow_dynamic_recursion: false,
            })
        );
    }

    #[test]
    fn push_artifact_rejects_root() {
        let (ctx, mut rx) = artifact_ctx(HashMap::new());
        let err = ctx
            .results()
            .push_artifact(
                "r",
                ArtifactSource::Bytes(vec![1]),
                ArtifactOpts::new(ArtifactRelation::Root),
            )
            .unwrap_err();
        assert!(matches!(err, SdkError::InvalidContext(_)));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn push_artifact_enforces_limit() {
        let config = HashMap::from([(job_config::MAX_ARTIFACTS.to_string(), "1".to_string())]);
        let (ctx, mut rx) = artifact_ctx(config);
        let opts = ArtifactOpts::new(ArtifactRelation::Extracted);
        ctx.results()
            .push_artifact("a", ArtifactSource::Bytes(vec![1]), opts)
            .unwrap();
        let err = ctx
            .results()
            .push_artifact("b", ArtifactSource::Bytes(vec![2]), opts)
            .unwrap_err();
        assert!(matches!(err, SdkError::ArtifactLimit { limit: 1 }));
        // Stored (non-analysed) artifacts do not count against the limit.
        ctx.results()
            .push_artifact(
                "c",
                ArtifactSource::Bytes(vec![3]),
                ArtifactOpts::new(ArtifactRelation::Output),
            )
            .unwrap();
        assert_eq!(rx.try_recv().unwrap().result_name, "a");
        assert_eq!(rx.try_recv().unwrap().result_name, "c");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn failed_push_artifact_does_not_consume_budget() {
        let config = HashMap::from([(job_config::MAX_ARTIFACTS.to_string(), "1".to_string())]);
        let (ctx, mut rx) = artifact_ctx(config);
        let opts = ArtifactOpts::new(ArtifactRelation::Extracted);
        let err = ctx
            .results()
            .push_artifact(
                "missing",
                ArtifactSource::File(PathBuf::from("/nonexistent/malbox-artifact")),
                opts,
            )
            .unwrap_err();
        assert!(matches!(err, SdkError::Io(_)));
        ctx.results()
            .push_artifact("a", ArtifactSource::Bytes(vec![1]), opts)
            .unwrap();
        assert_eq!(rx.try_recv().unwrap().result_name, "a");
    }

    #[test]
    fn file_result_without_stash_is_sent_inline() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("f.bin");
        std::fs::write(&path, b"hello").unwrap();
        let (ctx, mut rx) = artifact_ctx(HashMap::new());
        ctx.results().push_file("f", &path).unwrap();
        let msg = rx.try_recv().unwrap();
        assert_eq!(msg.kind, ResultKind::Result);
        assert_eq!(msg.result_name, "f");
        assert_eq!(msg.data, b"hello");
    }
}
