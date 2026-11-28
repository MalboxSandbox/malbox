//! Error types raised while discovering and watching plugins on disk.

use std::path::PathBuf;
use thiserror::Error;

/// Failure while constructing the plugin registry.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RegistryError {
    /// The initial scan of the plugin directory failed.
    #[error("failed to scan plugin directory: {0}")]
    Scan(#[from] ScanError),

    /// The filesystem watcher for hot-reload could not be started.
    #[error("failed to start filesystem watcher: {0}")]
    Watcher(#[from] WatcherError),
}

/// Failure while scanning the plugin directory or loading one plugin.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ScanError {
    /// The configured plugin directory does not exist.
    #[error("plugin directory does not exist: {}", .0.display())]
    DirectoryNotFound(PathBuf),

    /// Reading a directory or file failed.
    #[error("failed to read directory: {0}")]
    Io(#[from] std::io::Error),

    /// A plugin's manifest could not be parsed or validated.
    #[error("invalid manifest in {}: {source}", path.display())]
    Manifest {
        /// Path of the offending manifest file.
        path: PathBuf,
        /// Parser or validation error for the manifest.
        #[source]
        source: malbox_plugin_manifest::ManifestError,
    },
}

/// Failure in the filesystem watcher backing plugin hot-reload.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WatcherError {
    /// The `notify` backend reported an error.
    #[error("filesystem watcher error: {0}")]
    Notify(#[from] notify::Error),
}
