use super::error::{Result, StorageError};
use std::path::{Path, PathBuf};
use tracing::info;

/// File name of a stored plugin report.
const REPORT_FILE: &str = "report.json";

/// Filesystem storage for plugin reports.
///
/// Reports are stored at
/// `{data_dir}/results/{task_id}/{artifact_id}/{plugin_name}/report.json`.
/// Writes are atomic (tempfile + rename).
pub struct ResultStore {
    data_dir: PathBuf,
}

impl ResultStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            data_dir: data_dir.to_path_buf(),
        }
    }

    fn relative_report_path(task_id: i32, artifact_id: i64, plugin_name: &str) -> String {
        format!("results/{task_id}/{artifact_id}/{plugin_name}/{REPORT_FILE}")
    }

    /// Store a plugin's report JSON and return its path relative to the
    /// data directory (the value kept in `plugin_reports.report_path`).
    pub async fn store_report(
        &self,
        task_id: i32,
        artifact_id: i64,
        plugin_name: &str,
        data: &[u8],
    ) -> Result<String> {
        let relative = Self::relative_report_path(task_id, artifact_id, plugin_name);
        let dest = self.data_dir.join(&relative);

        let parent = dest.parent().ok_or_else(|| StorageError::PathError {
            message: "report path has no parent".into(),
            path: dest.clone(),
        })?;
        tokio::fs::create_dir_all(parent).await?;

        let tmp = tempfile::NamedTempFile::new_in(parent)?;
        tokio::fs::write(tmp.path(), data).await?;
        tmp.persist(&dest).map_err(|e| e.error)?;

        info!(
            task_id,
            artifact_id,
            plugin_name,
            path = %dest.display(),
            size = data.len(),
            "Report stored"
        );
        Ok(relative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_path_structure() {
        assert_eq!(
            ResultStore::relative_report_path(7, 42, "guest-yara-scanner"),
            "results/7/42/guest-yara-scanner/report.json"
        );
    }

    #[tokio::test]
    async fn store_report_writes_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = ResultStore::new(dir.path());

        let rel = store
            .store_report(1, 3, "test-plugin", b"{\"ok\":true}")
            .await
            .unwrap();
        assert_eq!(rel, "results/1/3/test-plugin/report.json");

        let written = std::fs::read(dir.path().join(&rel)).unwrap();
        assert_eq!(written, b"{\"ok\":true}");

        // Overwrites in place.
        store
            .store_report(1, 3, "test-plugin", b"{}")
            .await
            .unwrap();
        assert_eq!(std::fs::read(dir.path().join(&rel)).unwrap(), b"{}");
    }
}
