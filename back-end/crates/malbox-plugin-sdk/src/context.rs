//! Plugin execution context.
//!
//! [`Context`] is the main handle plugins use during task execution.
//! It provides access to task metadata via [`TaskInfo`], result pushing
//! via [`ResultSink`], and progress reporting back to the daemon.

pub mod message;
mod result_sink;

pub use result_sink::ResultSink;

use crate::error::{Result, SdkError};
use crate::result::PluginResult;
use message::{ResultFormat, ResultKind, TaskResultMessage};

use malbox_plugin_transport::messages::events::Event;
use malbox_plugin_transport::traits::TransportEmitter;
use malbox_plugin_transport::{DROP_RESULT_NAME, SHARED_RESULT_NAME, job_config};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use tracing::{info, warn};

#[cfg(feature = "guest")]
use crate::stash::ResultStash;

#[derive(Serialize)]
struct ProgressPayload<'a> {
    progress: f64,
    message: &'a str,
}

/// Sender half of the result channel between a plugin and its runtime.
///
/// The runtime creates one of these per task and hands it to [`Context`].
/// During `on_task` the context uses it to stream [`TaskResultMessage`]s
/// back to the daemon. It is `None` when no task is active.
pub type ResultSender = tokio::sync::mpsc::UnboundedSender<TaskResultMessage>;

pub(super) struct ContextInner {
    pub task_id: i32,
    pub sample_path: PathBuf,
    pub config: HashMap<String, String>,
    pub emitter: Arc<dyn TransportEmitter + Send + Sync>,
    pub result_tx: Mutex<Option<ResultSender>>,
    #[cfg(feature = "guest")]
    pub stash: Option<Arc<ResultStash>>,
    pub claimed_paths: Mutex<HashSet<PathBuf>>,
    /// Analysed artifacts pushed so far (checked against `malbox.max_artifacts`).
    pub analysed_pushes: AtomicU64,
}

fn lock_or_err<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>> {
    mutex
        .lock()
        .map_err(|_| SdkError::Channel("internal mutex poisoned".into()))
}

/// The main handle plugins interact with during task execution.
///
/// Provides access to task metadata, result pushing, and progress reporting.
/// `Context` is cheaply cloneable (it wraps an `Arc`) so you can share it
/// with background threads.
#[derive(Clone)]
pub struct Context {
    inner: Arc<ContextInner>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Context")
            .field("task_id", &self.inner.task_id)
            .field("sample_path", &self.inner.sample_path)
            .finish_non_exhaustive()
    }
}

impl Context {
    /// Create a new context with task data, a transport emitter, and an
    /// optional result channel.
    ///
    /// Used by the runtime; downstream test code should use
    /// [`Context::test_new`](crate::testkit) under the `testkit` feature.
    #[cfg(any(test, feature = "host", feature = "guest", feature = "testkit"))]
    pub(crate) fn new(
        task_id: i32,
        sample_path: PathBuf,
        config: HashMap<String, String>,
        emitter: Arc<dyn TransportEmitter + Send + Sync>,
        result_tx: Option<ResultSender>,
        #[cfg(feature = "guest")] stash: Option<Arc<ResultStash>>,
    ) -> Self {
        Self {
            inner: Arc::new(ContextInner {
                task_id,
                sample_path,
                config,
                emitter,
                result_tx: Mutex::new(result_tx),
                #[cfg(feature = "guest")]
                stash,
                claimed_paths: Mutex::new(HashSet::new()),
                analysed_pushes: AtomicU64::new(0),
            }),
        }
    }

    /// Get a read-only view of the current task's metadata.
    pub fn task(&self) -> TaskInfo<'_> {
        TaskInfo { inner: &self.inner }
    }

    /// Get a [`ResultSink`] for pushing results back to the daemon.
    pub fn results(&self) -> ResultSink<'_> {
        ResultSink { inner: &self.inner }
    }

    /// Report execution progress (0.0 to 1.0) with a status message.
    ///
    /// Progress updates are streamed to the daemon in real-time and can be
    /// shown in the UI. The `progress` value is clamped to `[0.0, 1.0]`.
    ///
    /// If a result channel is available, a PROGRESS-kind message is sent
    /// over it. Otherwise, just logs the progress.
    ///
    /// Never blocks: the result channel is unbounded.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError::Channel`] if the result channel's mutex is poisoned
    /// or the runtime has dropped the receiver, and
    /// [`SdkError::Serialization`] if the progress payload cannot be encoded.
    pub fn progress(&self, pct: f64, message: &str) -> Result<()> {
        let clamped = pct.clamp(0.0, 1.0);
        info!(kind = "progress", progress = clamped, %message, "Plugin progress");

        let tx = {
            let guard = lock_or_err(&self.inner.result_tx)?;
            guard.clone()
        };

        if let Some(tx) = tx {
            let progress_data = serde_json::to_vec(&ProgressPayload {
                progress: clamped,
                message,
            })?;

            let msg = TaskResultMessage {
                task_id: self.inner.task_id,
                result_name: String::new(),
                data: progress_data,
                format: ResultFormat::Json,
                is_final: false,
                kind: ResultKind::Progress,
                stash_handle: String::new(),
                stash_format: ResultFormat::Unspecified,
                stash_size: 0,
                artifact: None,
            };

            tx.send(msg)
                .map_err(|e| SdkError::Channel(format!("progress: {e}")))?;
        }

        Ok(())
    }

    /// Merge `{key: value}` into the task-wide shared data. Arrays are
    /// unioned with the existing value; anything else overwrites it.
    ///
    /// # Errors
    ///
    /// Fails like [`ResultSink::push`] when the result cannot be delivered.
    pub fn shared_put(&self, key: &str, value: serde_json::Value) -> Result<()> {
        let mut obj = serde_json::Map::new();
        obj.insert(key.to_string(), value);
        self.results().push(PluginResult::Json {
            name: SHARED_RESULT_NAME.to_string(),
            data: serde_json::to_vec(&obj)?,
        })
    }

    /// Ask the daemon to skip this artifact's later stages.
    ///
    /// # Errors
    ///
    /// Fails like [`ResultSink::push`] when the result cannot be delivered.
    pub fn drop_artifact(&self) -> Result<()> {
        self.results().push(PluginResult::Json {
            name: DROP_RESULT_NAME.to_string(),
            data: b"{}".to_vec(),
        })
    }

    /// Send a transport-level event to the daemon.
    ///
    /// Most plugins won't need this - it's an escape hatch for cases
    /// where you need to signal something outside the normal result flow.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError::Transport`] if the emitter fails to send the event.
    pub fn emit_event(&self, event: Event) -> Result<()> {
        self.inner.emitter.emit(event).map_err(SdkError::Transport)
    }

    /// Emit a warning via `tracing` at the WARN level.
    ///
    /// For guest plugins, warnings are captured by the [`LogBus`](crate::log::LogBus)
    /// and streamed to the daemon. They appear in the log stream, not in the
    /// task report - use [`ResultSink::push`] for structured results.
    ///
    /// # Errors
    ///
    /// Never fails; the `Result` is kept for API stability.
    pub fn warn(&self, message: &str) -> Result<()> {
        warn!(kind = "warning", %message, "Plugin warning");
        Ok(())
    }

    /// Mark a file path as already handled so auto-collection skips it.
    ///
    /// Call this when your plugin reads a file from the artifacts directory
    /// and sends its own processed version as a result. Without this, the
    /// auto-collector would send the raw file as a duplicate.
    pub fn mark_collected(&self, path: impl AsRef<Path>) {
        if let Ok(canonical) = std::fs::canonicalize(path.as_ref())
            && let Ok(mut guard) = self.inner.claimed_paths.lock()
        {
            guard.insert(canonical);
        }
    }

    /// Close the result channel, preventing further pushes.
    ///
    /// Called by the runtime after `on_stop` to signal end-of-task.
    #[cfg_attr(
        not(any(test, feature = "host")),
        expect(dead_code, reason = "only the host runtime closes the channel")
    )]
    pub(crate) fn close_result_channel(&self) {
        if let Ok(mut guard) = self.inner.result_tx.lock() {
            *guard = None;
        }
    }

    /// Return a snapshot of all claimed file paths.
    ///
    /// Used internally by auto-collection to determine which files
    /// have already been explicitly sent or marked.
    #[cfg(feature = "guest")]
    pub(crate) fn claimed_paths(&self) -> HashSet<PathBuf> {
        self.inner
            .claimed_paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

/// Read-only view of task metadata: the task ID, sample file path, and
/// key-value configuration submitted with the task.
pub struct TaskInfo<'a> {
    inner: &'a ContextInner,
}

impl std::fmt::Debug for TaskInfo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskInfo")
            .field("id", &self.inner.task_id)
            .field("sample_path", &self.inner.sample_path)
            .finish_non_exhaustive()
    }
}

impl TaskInfo<'_> {
    /// The task's numeric identifier, unique within a single daemon run.
    pub fn id(&self) -> i32 {
        self.inner.task_id
    }

    /// Path to the sample file on disk.
    pub fn sample_path(&self) -> &Path {
        &self.inner.sample_path
    }

    /// Read the entire sample file into memory. Be mindful of large samples.
    ///
    /// # Errors
    ///
    /// Returns [`SdkError::Io`] if the sample file cannot be read.
    pub fn sample_bytes(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.inner.sample_path).map_err(SdkError::Io)
    }

    /// The full key-value configuration map submitted with the task.
    pub fn config(&self) -> &HashMap<String, String> {
        &self.inner.config
    }

    /// Look up a single value from the task configuration. Returns `None`
    /// if the key is not present.
    pub fn config_value(&self, key: &str) -> Option<&str> {
        self.inner.config.get(key).map(String::as_str)
    }

    /// The orchestrator job id, when run by the daemon.
    pub fn job_id(&self) -> Option<i64> {
        self.config_value(job_config::JOB_ID)?.parse().ok()
    }

    /// The artifact this job analyses. `None` unless the daemon sent
    /// `malbox.artifact_id`.
    pub fn artifact(&self) -> Option<ArtifactInfo> {
        let id = self.config_value(job_config::ARTIFACT_ID)?.parse().ok()?;
        let get = |k: &str| self.config_value(k).unwrap_or_default().to_string();
        Some(ArtifactInfo {
            id,
            sha256: get(job_config::ARTIFACT_SHA256),
            artifact_type: get(job_config::ARTIFACT_TYPE),
            name: get(job_config::ARTIFACT_NAME),
            depth: self
                .config_value(job_config::ARTIFACT_DEPTH)
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        })
    }

    /// Whether the submitter asked for a deep scan.
    pub fn deep_scan(&self) -> bool {
        self.config_value(job_config::DEEP_SCAN) == Some("true")
    }

    /// Ancestors of the current artifact, root first.
    pub fn ancestry(&self) -> Vec<AncestryEntry> {
        self.json_value(job_config::ANCESTRY)
    }

    /// Task-wide shared data written by earlier plugins.
    pub fn shared(&self) -> serde_json::Map<String, serde_json::Value> {
        self.json_value(job_config::SHARED)
    }

    /// Indicators reported earlier for this artifact and its ancestors.
    pub fn indicators(&self) -> Vec<crate::report::Indicator> {
        self.json_value(job_config::INDICATORS)
    }

    /// Resolved plugin parameters (manifest defaults plus submission overrides).
    pub fn params(&self) -> serde_json::Map<String, serde_json::Value> {
        self.json_value(job_config::PARAMS)
    }

    fn json_value<T: DeserializeOwned + Default>(&self, key: &str) -> T {
        let Some(raw) = self.config_value(key) else {
            return T::default();
        };
        serde_json::from_str(raw).unwrap_or_else(|e| {
            warn!(key, error = %e, "invalid JSON in job config");
            T::default()
        })
    }
}

/// The artifact a job analyses.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ArtifactInfo {
    /// Database id of the artifact.
    pub id: i64,
    /// Hex SHA-256 of the artifact's content.
    pub sha256: String,
    /// Detected artifact type.
    pub artifact_type: String,
    /// File name the artifact was submitted or pushed under.
    pub name: String,
    /// Distance from the submitted sample (the root is 0).
    pub depth: u32,
}

/// One ancestor of the current artifact.
#[derive(Debug, Clone, serde::Deserialize)]
#[non_exhaustive]
pub struct AncestryEntry {
    /// Hex SHA-256 of the ancestor's content.
    pub sha256: String,
    /// Detected type of the ancestor.
    pub artifact_type: String,
    /// Database label of how the ancestor relates to its own parent
    /// (see [`ArtifactRelation::as_str`](crate::ArtifactRelation::as_str)).
    pub relation: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

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
            #[cfg(feature = "guest")]
            None,
        );
        assert_eq!(ctx.task().id(), 7);
        assert_eq!(ctx.task().sample_path(), Path::new("/tmp/s.bin"));
        assert_eq!(ctx.task().config().get("k"), Some(&"v".to_string()));
        assert_eq!(ctx.task().config_value("k"), Some("v"));
        assert_eq!(ctx.task().config_value("missing"), None);
    }
}
