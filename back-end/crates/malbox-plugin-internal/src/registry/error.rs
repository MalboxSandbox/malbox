use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("failed to scan plugin directory: {0}")]
    Scan(#[from] ScanError),

    #[error("failed to start filesystem watcher: {0}")]
    Watcher(#[from] WatcherError),
}

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("plugin directory does not exist: {}", .0.display())]
    DirectoryNotFound(PathBuf),

    #[error("failed to read directory: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid manifest in {}: {source}", path.display())]
    Manifest {
        path: PathBuf,
        #[source]
        source: malbox_plugin_manifest::ManifestError,
    },
}

#[derive(Debug, Error)]
pub enum WatcherError {
    #[error("filesystem watcher error: {0}")]
    Notify(#[from] notify::Error),
}
