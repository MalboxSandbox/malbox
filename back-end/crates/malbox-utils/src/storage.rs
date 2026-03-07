pub mod error;
pub mod paths;

use error::{Result, StorageError};
use std::path::{Path, PathBuf};

/// Content-addressed sample file storage.
///
/// Files are stored at `{base_dir}/{sha256[0:2]}/{sha256[2:4]}/{sha256}`.
/// Writes are atomic (tempfile + rename) and idempotent (skip if exists).
pub struct SampleStore {
    base_dir: PathBuf,
}

impl SampleStore {
    /// Create a new store rooted at `{data_dir}/samples`.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            base_dir: data_dir.join("samples"),
        }
    }

    /// Resolve the on-disk path for a given SHA256 hash.
    pub fn path(&self, sha256: &str) -> Result<PathBuf> {
        if sha256.len() < 4 {
            return Err(StorageError::InvalidHash(sha256.len()));
        }
        Ok(self
            .base_dir
            .join(&sha256[0..2])
            .join(&sha256[2..4])
            .join(sha256))
    }

    /// Check whether a sample exists on disk.
    pub async fn exists(&self, sha256: &str) -> Result<bool> {
        let path = self.path(sha256)?;
        Ok(tokio::fs::try_exists(&path).await.unwrap_or(false))
    }

    /// Store sample bytes at the content-addressed path.
    ///
    /// Idempotent: if a file with this hash already exists, the write is skipped.
    /// Atomic: writes to a tempfile first, then renames into place.
    pub async fn store(&self, sha256: &str, data: &[u8]) -> Result<PathBuf> {
        let dest = self.path(sha256)?;

        if tokio::fs::try_exists(&dest).await.unwrap_or(false) {
            tracing::debug!(sha256, "Sample already exists, skipping write");
            return Ok(dest);
        }

        let parent = dest
            .parent()
            .expect("content-addressed path always has a parent");
        tokio::fs::create_dir_all(parent).await?;

        // Atomic write: tempfile in the same directory, then rename.
        let tmp = tempfile::NamedTempFile::new_in(parent)?;
        tokio::fs::write(tmp.path(), data).await?;
        tmp.persist(&dest).map_err(|e| e.error)?;

        tracing::info!(sha256, path = %dest.display(), "Sample stored");
        Ok(dest)
    }
}
