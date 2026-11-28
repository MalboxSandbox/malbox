use std::path::PathBuf;
use thiserror::Error;

/// Failures raised by sample and report storage operations.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum StorageError {
    /// A required path does not exist.
    #[error("Path not found: {0}")]
    NotFound(PathBuf),

    /// A path could not be used (e.g. it has no parent directory).
    #[error("Path error: {message} for {path}")]
    PathError {
        /// What went wrong with the path.
        message: String,
        /// The offending path.
        path: PathBuf,
    },

    /// Underlying filesystem I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// XDG base directory could not be resolved.
    #[error("XDG error: {0}")]
    Xdg(String),

    /// SHA256 hash too short to derive a sharded path; holds its length.
    #[error("Invalid SHA256 hash: must be at least 4 characters, got {0}")]
    InvalidHash(usize),
}

/// Result alias for storage operations.
pub type Result<T> = std::result::Result<T, StorageError>;
