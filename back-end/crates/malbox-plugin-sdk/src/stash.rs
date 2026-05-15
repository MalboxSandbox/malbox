//! Disk-backed result stash for large guest-plugin results.
//!
//! Large results (above a configurable threshold) are written to a temp
//! file under `<work_dir>/_stash/`, keyed by an opaque handle. The daemon
//! pulls them via the `PullResult` gRPC RPC. SDK-written temp files are
//! deleted after a successful pull; plugin-owned files (from
//! `PluginResult::File`) are referenced by path and left alone.

use crate::error::{Result, SdkError};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Opaque identifier for a stashed result. A UUID v4 string.
pub type Handle = String;

/// On-wire format hint, kept in sync with `proto::ResultFormat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StashFormat {
    Json,
    Bytes,
}

/// Metadata for a single stashed result (path on disk, ownership, size).
#[derive(Debug, Clone)]
pub struct StashEntry {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub result_name: String,
    #[allow(dead_code)]
    pub format: StashFormat,
    pub size_bytes: u64,
    /// True for SDK-written temp files (deleted after pull). False for
    /// `PluginResult::File` references (left alone).
    pub sdk_owned: bool,
    pub task_id: i32,
    pub created_at: Instant,
}

/// Controls when results are stashed to disk and how long they live.
#[derive(Debug, Clone)]
pub struct StashConfig {
    /// Results above this many bytes go to disk + ref. Below stays inline.
    pub threshold_bytes: usize,
    /// Maximum age of an un-pulled entry before the TTL sweep reclaims it.
    pub ttl: Duration,
}

impl Default for StashConfig {
    fn default() -> Self {
        Self {
            threshold_bytes: 1024 * 1024, // 1 MB
            ttl: Duration::from_secs(120),
        }
    }
}

/// Disk-backed result stash.
pub struct ResultStash {
    stash_dir: PathBuf,
    config: StashConfig,
    entries: Mutex<HashMap<Handle, StashEntry>>,
}

impl ResultStash {
    /// Create a new stash rooted at `stash_dir`. Creates the directory if
    /// missing.
    pub fn new(stash_dir: PathBuf, config: StashConfig) -> Result<Self> {
        std::fs::create_dir_all(&stash_dir).map_err(SdkError::Io)?;
        Ok(Self {
            stash_dir,
            config,
            entries: Mutex::new(HashMap::new()),
        })
    }

    /// Return the stash configuration.
    pub fn config(&self) -> &StashConfig {
        &self.config
    }

    /// Insert an in-memory payload. Writes it to a new temp file in the
    /// stash directory and returns the handle.
    pub fn insert_bytes(
        &self,
        task_id: i32,
        result_name: String,
        format: StashFormat,
        data: Vec<u8>,
    ) -> Result<Handle> {
        let handle = uuid::Uuid::new_v4().to_string();
        let path = self.stash_dir.join(format!("{handle}.bin"));
        std::fs::write(&path, &data).map_err(SdkError::Io)?;
        let size_bytes = data.len() as u64;

        let entry = StashEntry {
            path,
            result_name,
            format,
            size_bytes,
            sdk_owned: true,
            task_id,
            created_at: Instant::now(),
        };
        self.entries
            .lock()
            .map_err(|_| SdkError::Channel("stash mutex poisoned".into()))?
            .insert(handle.clone(), entry);
        Ok(handle)
    }

    /// Insert a reference to an existing file (plugin-owned). The file is
    /// not copied or touched, and is never deleted by the stash.
    pub fn insert_file(
        &self,
        task_id: i32,
        result_name: String,
        format: StashFormat,
        path: PathBuf,
    ) -> Result<Handle> {
        let size_bytes = std::fs::metadata(&path).map_err(SdkError::Io)?.len();
        let handle = uuid::Uuid::new_v4().to_string();
        let entry = StashEntry {
            path,
            result_name,
            format,
            size_bytes,
            sdk_owned: false,
            task_id,
            created_at: Instant::now(),
        };
        self.entries
            .lock()
            .map_err(|_| SdkError::Channel("stash mutex poisoned".into()))?
            .insert(handle.clone(), entry);
        Ok(handle)
    }

    /// Take the entry for `handle`, removing it from the map.
    ///
    /// The returned entry's file has **not** been deleted - the caller
    /// (typically `on_pull_result`) reads it, then calls
    /// [`Self::cleanup_after_pull`] to delete SDK-owned files.
    pub fn take(&self, handle: &str) -> Option<StashEntry> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(handle)
    }

    /// Return `size_bytes` for `handle` without removing the entry.
    pub fn peek_size(&self, handle: &str) -> Option<u64> {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(handle)
            .map(|e| e.size_bytes)
    }

    /// Delete the SDK-written temp file for a pulled entry. No-op for
    /// plugin-owned files.
    pub fn cleanup_after_pull(&self, entry: &StashEntry) {
        if entry.sdk_owned {
            let _ = std::fs::remove_file(&entry.path);
        }
    }

    /// Remove all entries for a finished task. SDK-owned files are deleted;
    /// plugin-owned files are left alone. Returns the number of entries
    /// reclaimed.
    #[allow(dead_code)]
    pub fn sweep_task(&self, task_id: i32) -> usize {
        self.sweep_where(|e| e.task_id == task_id)
    }

    /// Remove entries older than `self.config.ttl`. SDK-owned files deleted;
    /// plugin-owned files left alone. Returns the number of entries
    /// reclaimed.
    pub fn sweep_expired(&self) -> usize {
        let ttl = self.config.ttl;
        let now = Instant::now();
        self.sweep_where(|e| now.saturating_duration_since(e.created_at) > ttl)
    }

    fn sweep_where(&self, predicate: impl Fn(&StashEntry) -> bool) -> usize {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let handles: Vec<Handle> = entries
            .iter()
            .filter_map(|(h, e)| if predicate(e) { Some(h.clone()) } else { None })
            .collect();
        let n = handles.len();
        for h in handles {
            if let Some(entry) = entries.remove(&h)
                && entry.sdk_owned
            {
                let _ = std::fs::remove_file(&entry.path);
            }
        }
        n
    }

    /// Sweep any files in `stash_dir` that are not tracked in `entries`.
    /// Called once on startup to clean up orphans from a previous crashed
    /// run. Does not touch files referenced by live entries (which is
    /// always empty on startup).
    pub fn sweep_orphans_on_startup(stash_dir: &Path) -> std::io::Result<usize> {
        if !stash_dir.exists() {
            return Ok(0);
        }
        let mut n = 0;
        for entry in std::fs::read_dir(stash_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let _ = std::fs::remove_file(&path);
                n += 1;
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_stash() -> (tempfile::TempDir, ResultStash) {
        let dir = tempfile::tempdir().unwrap();
        let stash_path = dir.path().join("_stash");
        let stash = ResultStash::new(stash_path, StashConfig::default()).unwrap();
        (dir, stash)
    }

    #[test]
    fn insert_bytes_writes_file_and_returns_handle() {
        let (_dir, stash) = temp_stash();
        let handle = stash
            .insert_bytes(1, "r".into(), StashFormat::Bytes, vec![1, 2, 3])
            .unwrap();
        assert!(!handle.is_empty());
        let entry = stash.take(&handle).expect("entry should exist");
        assert_eq!(entry.size_bytes, 3);
        assert!(entry.sdk_owned);
        let contents = std::fs::read(&entry.path).unwrap();
        assert_eq!(contents, vec![1, 2, 3]);
    }

    #[test]
    fn insert_file_records_path_without_copy() {
        let dir = tempfile::tempdir().unwrap();
        let plugin_file = dir.path().join("artifact.bin");
        std::fs::write(&plugin_file, b"hello world").unwrap();

        let stash_dir = dir.path().join("_stash");
        let stash = ResultStash::new(stash_dir, StashConfig::default()).unwrap();
        let handle = stash
            .insert_file(7, "cap".into(), StashFormat::Bytes, plugin_file.clone())
            .unwrap();

        let entry = stash.take(&handle).unwrap();
        assert_eq!(entry.path, plugin_file);
        assert_eq!(entry.size_bytes, b"hello world".len() as u64);
        assert!(!entry.sdk_owned);
    }

    #[test]
    fn cleanup_after_pull_deletes_only_sdk_owned_files() {
        let (_dir, stash) = temp_stash();
        let h1 = stash
            .insert_bytes(1, "r1".into(), StashFormat::Json, vec![9])
            .unwrap();
        let e1 = stash.take(&h1).unwrap();
        assert!(e1.path.exists());
        stash.cleanup_after_pull(&e1);
        assert!(!e1.path.exists());

        // Plugin-owned file: untouched.
        let outer = tempfile::tempdir().unwrap();
        let plugin_file = outer.path().join("keep.bin");
        std::fs::write(&plugin_file, b"x").unwrap();
        let h2 = stash
            .insert_file(2, "r2".into(), StashFormat::Bytes, plugin_file.clone())
            .unwrap();
        let e2 = stash.take(&h2).unwrap();
        stash.cleanup_after_pull(&e2);
        assert!(
            plugin_file.exists(),
            "plugin-owned file must not be deleted"
        );
    }

    #[test]
    fn sweep_task_removes_all_entries_for_task() {
        let (_dir, stash) = temp_stash();
        let h1 = stash
            .insert_bytes(5, "a".into(), StashFormat::Bytes, vec![1])
            .unwrap();
        let _ = stash
            .insert_bytes(5, "b".into(), StashFormat::Bytes, vec![2])
            .unwrap();
        let _ = stash
            .insert_bytes(6, "c".into(), StashFormat::Bytes, vec![3])
            .unwrap();

        let reclaimed = stash.sweep_task(5);
        assert_eq!(reclaimed, 2);

        // Handle h1 should be gone.
        assert!(stash.take(&h1).is_none());
    }

    #[test]
    fn sweep_expired_reclaims_old_entries() {
        let dir = tempfile::tempdir().unwrap();
        let stash = ResultStash::new(
            dir.path().join("_stash"),
            StashConfig {
                threshold_bytes: 1024,
                ttl: Duration::from_millis(1),
            },
        )
        .unwrap();

        let _ = stash
            .insert_bytes(1, "r".into(), StashFormat::Bytes, vec![0])
            .unwrap();
        std::thread::sleep(Duration::from_millis(10));

        let reclaimed = stash.sweep_expired();
        assert_eq!(reclaimed, 1);
    }

    #[test]
    fn sweep_orphans_deletes_loose_files() {
        let dir = tempfile::tempdir().unwrap();
        let stash_dir = dir.path().join("_stash");
        std::fs::create_dir_all(&stash_dir).unwrap();
        std::fs::write(stash_dir.join("orphan1.bin"), b"x").unwrap();
        std::fs::write(stash_dir.join("orphan2.bin"), b"y").unwrap();

        let n = ResultStash::sweep_orphans_on_startup(&stash_dir).unwrap();
        assert_eq!(n, 2);
        assert!(stash_dir.read_dir().unwrap().next().is_none());
    }
}
