//! The Task type passed to `Plugin::on_task`.

use crate::error::{Result, SdkError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Represents an analysis task assigned to this plugin.
#[non_exhaustive]
pub struct Task {
    pub(crate) id: i32,
    pub(crate) sample_path: PathBuf,
    pub(crate) config: HashMap<String, String>,
}

impl Task {
    /// Create a new task. Used by the runtime; downstream test code should use
    /// [`Task::test_new`](crate::testkit) under the `testkit` feature.
    pub(crate) fn new(id: i32, sample_path: PathBuf, config: HashMap<String, String>) -> Self {
        Self {
            id,
            sample_path,
            config,
        }
    }

    /// Return the task's numeric identifier.
    pub fn id(&self) -> i32 {
        self.id
    }

    /// Get the path to the sample file.
    pub fn sample_path(&self) -> &Path {
        &self.sample_path
    }

    /// Return the task's configuration map.
    pub fn config(&self) -> &HashMap<String, String> {
        &self.config
    }

    /// Read the entire sample file into memory.
    pub fn sample_bytes(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.sample_path).map_err(SdkError::Io)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn task_exposes_getters() {
        let mut cfg = HashMap::new();
        cfg.insert("k".to_string(), "v".to_string());

        let task = Task::new(7, PathBuf::from("/tmp/s.bin"), cfg);

        assert_eq!(task.id(), 7);
        assert_eq!(task.sample_path(), Path::new("/tmp/s.bin"));
        assert_eq!(task.config().get("k"), Some(&"v".to_string()));
    }
}
