use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Path not found: {0}")]
    NotFound(PathBuf),

    #[error("Path error: {message} for {path}")]
    PathError { message: String, path: PathBuf },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("XDG error: {0}")]
    Xdg(String),

    #[error("Invalid SHA256 hash: must be at least 4 characters, got {0}")]
    InvalidHash(usize),
}

pub type Result<T> = std::result::Result<T, StorageError>;
