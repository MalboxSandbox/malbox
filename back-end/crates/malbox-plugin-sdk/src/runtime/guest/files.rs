//! File push/pull and path resolution helpers for the guest runtime.
//!
//! All paths supplied by the daemon are resolved against the runtime's
//! work directory. Absolute paths and `..` traversal are rejected.

use std::path::{Component, Path, PathBuf};

/// Push file contents into the work directory under `dest`.
pub(super) async fn push_file(
    work_dir: &Path,
    dest: &str,
    data: Vec<u8>,
) -> std::result::Result<(), String> {
    let path = resolve_path(work_dir, dest)?;
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("failed to create parent dirs: {}", e))?;
    }
    tokio::fs::write(&path, &data)
        .await
        .map_err(|e| format!("failed to write file: {}", e))
}

/// Read a file from the work directory.
pub(super) async fn pull_file(
    work_dir: &Path,
    source: &str,
) -> std::result::Result<Vec<u8>, String> {
    let path = resolve_path(work_dir, source)?;
    tokio::fs::read(&path)
        .await
        .map_err(|e| format!("failed to read file: {}", e))
}

/// Resolve a relative path against `work_dir`, rejecting traversal attempts.
///
/// - Absolute paths are rejected.
/// - `..` components that would escape `work_dir` are rejected.
pub(super) fn resolve_path(
    work_dir: &Path,
    relative: &str,
) -> std::result::Result<PathBuf, String> {
    if Path::new(relative).is_absolute() {
        return Err(format!("absolute paths not allowed: {}", relative));
    }

    let joined = work_dir.join(relative);
    let normalized = normalize_path(&joined);
    let normalized_work_dir = normalize_path(work_dir);

    if !normalized.starts_with(&normalized_work_dir) {
        return Err(format!("path escapes work directory: {}", relative));
    }

    Ok(normalized)
}

/// Normalize a path by resolving `.` and `..` components without filesystem access.
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                if matches!(components.last(), Some(Component::Normal(_))) {
                    components.pop();
                }
            }
            Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- normalize_path tests --

    #[test]
    fn normalize_path_resolves_dot_components() {
        let result = normalize_path(&PathBuf::from("/a/b/./c"));
        assert_eq!(result, PathBuf::from("/a/b/c"));
    }

    #[test]
    fn normalize_path_resolves_dotdot_components() {
        let result = normalize_path(&PathBuf::from("/a/b/../c"));
        assert_eq!(result, PathBuf::from("/a/c"));
    }

    #[test]
    fn normalize_path_handles_trailing_dotdot() {
        let result = normalize_path(&PathBuf::from("/a/b/.."));
        assert_eq!(result, PathBuf::from("/a"));
    }

    // -- resolve_path tests --

    #[test]
    fn resolve_path_accepts_simple_relative() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_path(dir.path(), "samples/test.exe");
        assert!(result.is_ok());
        assert!(result.unwrap().starts_with(dir.path()));
    }

    #[test]
    fn resolve_path_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_path(dir.path(), "../etc/passwd");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("escapes work directory"));
    }

    #[test]
    fn resolve_path_rejects_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_path(dir.path(), "/etc/passwd");
        assert!(result.is_err());
    }

    // -- push_file / pull_file tests --

    #[tokio::test]
    async fn push_file_writes_to_work_dir() {
        let dir = tempfile::tempdir().unwrap();
        let result = push_file(dir.path(), "samples/test.exe", b"MZ\x90\x00".to_vec()).await;
        assert!(result.is_ok());

        let written = std::fs::read(dir.path().join("samples/test.exe")).unwrap();
        assert_eq!(written, b"MZ\x90\x00");
    }

    #[tokio::test]
    async fn push_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let result = push_file(dir.path(), "../escape.txt", b"bad".to_vec()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pull_file_reads_from_work_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("results/output.json");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, b"{\"key\": \"value\"}").unwrap();

        let result = pull_file(dir.path(), "results/output.json").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), b"{\"key\": \"value\"}");
    }

    #[tokio::test]
    async fn pull_file_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let result = pull_file(dir.path(), "../../etc/passwd").await;
        assert!(result.is_err());
    }
}
