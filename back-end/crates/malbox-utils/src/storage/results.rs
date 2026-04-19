use super::error::{Result, StorageError};
use std::path::{Path, PathBuf};
use tracing::info;

/// Format of a stored plugin result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultFormat {
    Json,
    Bytes,
}

impl ResultFormat {
    /// File extension for this format.
    pub fn extension(self) -> &'static str {
        match self {
            ResultFormat::Json => "json",
            ResultFormat::Bytes => "bin",
        }
    }

    /// Database enum value.
    pub fn as_db_str(self) -> &'static str {
        match self {
            ResultFormat::Json => "json",
            ResultFormat::Bytes => "bytes",
        }
    }
}

/// Filesystem storage for task analysis results.
///
/// Results are stored at `{base_dir}/{task_id}/{plugin_name}/{result_name}.{ext}`.
/// Writes are atomic (tempfile + rename).
pub struct ResultStore {
    base_dir: PathBuf,
}

impl ResultStore {
    /// Create a new store rooted at `{data_dir}/results`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            base_dir: data_dir.join("results"),
        }
    }

    /// Resolve the on-disk path for a result.
    pub fn path(
        &self,
        task_id: i32,
        plugin_name: &str,
        result_name: &str,
        format: ResultFormat,
    ) -> PathBuf {
        self.base_dir
            .join(task_id.to_string())
            .join(plugin_name)
            .join(format!("{}.{}", result_name, format.extension()))
    }

    /// Relative path from `data_dir` for storing in the database.
    pub fn relative_path(
        task_id: i32,
        plugin_name: &str,
        result_name: &str,
        format: ResultFormat,
    ) -> String {
        format!(
            "results/{}/{}/{}.{}",
            task_id,
            plugin_name,
            result_name,
            format.extension()
        )
    }

    /// Store result data and return the relative path for the DB pointer.
    ///
    /// Atomic: writes to a tempfile first, then renames into place.
    pub async fn store(
        &self,
        task_id: i32,
        plugin_name: &str,
        result_name: &str,
        format: ResultFormat,
        data: &[u8],
    ) -> Result<String> {
        let dest = self.path(task_id, plugin_name, result_name, format);

        let parent = dest.parent().ok_or_else(|| StorageError::PathError {
            message: "result path has no parent".into(),
            path: dest.clone(),
        })?;
        tokio::fs::create_dir_all(parent).await?;

        let tmp = tempfile::NamedTempFile::new_in(parent)?;
        tokio::fs::write(tmp.path(), data).await?;
        tmp.persist(&dest).map_err(|e| e.error)?;

        let relative = Self::relative_path(task_id, plugin_name, result_name, format);
        info!(
            task_id,
            plugin_name,
            result_name,
            path = %dest.display(),
            size = data.len(),
            "Result stored"
        );

        Ok(relative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_structure() {
        let store = ResultStore::new(Path::new("/data"));
        let path = store.path(7, "guest-yara-scanner", "yara_matches", ResultFormat::Json);
        assert_eq!(
            path,
            PathBuf::from("/data/results/7/guest-yara-scanner/yara_matches.json")
        );
    }

    #[test]
    fn relative_path_structure() {
        let rel =
            ResultStore::relative_path(7, "guest-yara-scanner", "yara_matches", ResultFormat::Json);
        assert_eq!(rel, "results/7/guest-yara-scanner/yara_matches.json");
    }

    #[test]
    fn bytes_format_uses_bin_extension() {
        let rel = ResultStore::relative_path(7, "plugin", "dump", ResultFormat::Bytes);
        assert_eq!(rel, "results/7/plugin/dump.bin");
    }

    #[tokio::test]
    async fn store_writes_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = ResultStore::new(dir.path());

        let rel = store
            .store(
                1,
                "test-plugin",
                "output",
                ResultFormat::Json,
                b"{\"ok\":true}",
            )
            .await
            .unwrap();

        assert_eq!(rel, "results/1/test-plugin/output.json");

        let written = std::fs::read(dir.path().join(&rel)).unwrap();
        assert_eq!(written, b"{\"ok\":true}");
    }
}
