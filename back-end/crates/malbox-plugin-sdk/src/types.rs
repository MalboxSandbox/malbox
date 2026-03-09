//! Core types for plugin metadata, tasks, results, and health status.

use crate::error::{Result, SdkError};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Metadata about a plugin.
///
/// Generic fields (`name`, `version`, `description`, `authors`) are sourced
/// automatically from the crate's `Cargo.toml` via `env!()` macros so that
/// plugin authors don't have to duplicate them in the `#[malbox(…)]` attribute.
#[derive(Debug, Clone)]
pub struct PluginMeta {
    pub name: &'static str,
    pub version: &'static str,
    pub description: Option<&'static str>,
    pub authors: &'static str,
    pub plugin_type: PluginType,
    pub state: PluginState,
    pub execution: ExecutionContext,
}

/// How the plugin communicates with the daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginType {
    /// Runs on the daemon host, communicating over IPC.
    Host,
    /// Runs inside a guest VM/container, communicating over gRPC.
    Guest,
}

/// Lifecycle behavior of the plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Stays running between tasks.
    Persistent,
    /// Spun up per task and torn down immediately after.
    Ephemeral,
    /// Lives for the duration of an analysis scope (e.g. a batch).
    Scoped,
}

/// Concurrency model for task execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionContext {
    /// Only one instance runs at a time across the entire daemon.
    Exclusive,
    /// Tasks are dispatched one at a time in order.
    Sequential,
    /// Multiple tasks may run concurrently.
    Parallel,
    /// No constraints on concurrency or ordering.
    Unrestricted,
}

/// Represents an analysis task assigned to this plugin.
pub struct Task {
    /// Unique task identifier within the daemon.
    pub id: i32,
    sample_path: PathBuf,
    /// Key-value configuration provided when the task was submitted.
    pub config: HashMap<String, String>,
}

impl Task {
    /// Create a new task (used internally by the runtime).
    pub fn new(id: i32, sample_path: PathBuf, config: HashMap<String, String>) -> Self {
        Self {
            id,
            sample_path,
            config,
        }
    }

    /// Get the path to the sample file.
    pub fn sample_path(&self) -> &Path {
        &self.sample_path
    }

    /// Read the entire sample file into memory.
    pub fn sample_bytes(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.sample_path).map_err(SdkError::Io)
    }
}

/// A named result produced by plugin analysis.
///
/// Each variant carries a `name` that identifies the result in the task
/// report (e.g. `"yara_matches"`, `"extracted_pe"`).
#[derive(Debug)]
pub enum PluginResult {
    /// Structured data serialized as JSON.
    Json { name: String, data: Vec<u8> },
    /// Arbitrary binary data.
    Bytes { name: String, data: Vec<u8> },
    /// A file on disk — the runtime streams it back to the daemon.
    File { name: String, path: PathBuf },
}

impl PluginResult {
    /// Create a JSON result from any serializable value.
    pub fn json(name: &str, value: &impl Serialize) -> Result<Self> {
        let data = serde_json::to_vec(value)?;
        Ok(PluginResult::Json {
            name: name.to_string(),
            data,
        })
    }

    /// Create a raw bytes result.
    pub fn bytes(name: &str, data: Vec<u8>) -> Self {
        PluginResult::Bytes {
            name: name.to_string(),
            data,
        }
    }

    /// Create a file result (the runtime will read and stream the file).
    pub fn file(name: &str, path: impl Into<PathBuf>) -> Self {
        PluginResult::File {
            name: name.to_string(),
            path: path.into(),
        }
    }

    /// Get the result name.
    pub fn name(&self) -> &str {
        match self {
            PluginResult::Json { name, .. } => name,
            PluginResult::Bytes { name, .. } => name,
            PluginResult::File { name, .. } => name,
        }
    }
}

/// Health status returned by optional health check handlers.
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub ready: bool,
    pub reason: String,
}

impl HealthStatus {
    /// Create a status indicating the plugin is ready to accept tasks.
    pub fn ready() -> Self {
        Self {
            ready: true,
            reason: String::new(),
        }
    }

    /// Create a status indicating the plugin is **not** ready, with a reason.
    pub fn not_ready(reason: &str) -> Self {
        Self {
            ready: false,
            reason: reason.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct TestData {
        key: String,
        value: i32,
    }

    #[test]
    fn plugin_result_json_serializes_correctly() {
        let data = TestData {
            key: "hello".into(),
            value: 42,
        };
        let result = PluginResult::json("test_result", &data).unwrap();
        assert_eq!(result.name(), "test_result");
        match result {
            PluginResult::Json { data, .. } => {
                let parsed: serde_json::Value = serde_json::from_slice(&data).unwrap();
                assert_eq!(parsed["key"], "hello");
                assert_eq!(parsed["value"], 42);
            }
            _ => panic!("expected Json variant"),
        }
    }

    #[test]
    fn plugin_result_bytes_stores_raw_data() {
        let result = PluginResult::bytes("raw", vec![0xDE, 0xAD]);
        assert_eq!(result.name(), "raw");
        match result {
            PluginResult::Bytes { data, .. } => assert_eq!(data, vec![0xDE, 0xAD]),
            _ => panic!("expected Bytes variant"),
        }
    }

    #[test]
    fn plugin_result_file_stores_path() {
        let result = PluginResult::file("capture", "/tmp/out.pcap");
        assert_eq!(result.name(), "capture");
        match result {
            PluginResult::File { path, .. } => assert_eq!(path, PathBuf::from("/tmp/out.pcap")),
            _ => panic!("expected File variant"),
        }
    }

    #[test]
    fn task_sample_bytes_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let sample_path = dir.path().join("sample.bin");
        std::fs::write(&sample_path, b"MZ\x90\x00").unwrap();

        let task = Task::new(1, sample_path.clone(), HashMap::new());
        assert_eq!(task.sample_path(), &sample_path);
        assert_eq!(task.sample_bytes().unwrap(), b"MZ\x90\x00");
    }

    #[test]
    fn task_sample_bytes_returns_error_on_missing_file() {
        let task = Task::new(1, PathBuf::from("/nonexistent/sample.bin"), HashMap::new());
        assert!(task.sample_bytes().is_err());
    }

    #[test]
    fn health_status_ready() {
        let status = HealthStatus::ready();
        assert!(status.ready);
        assert!(status.reason.is_empty());
    }

    #[test]
    fn health_status_not_ready() {
        let status = HealthStatus::not_ready("loading rules");
        assert!(!status.ready);
        assert_eq!(status.reason, "loading rules");
    }
}
