//! Error type returned by the plugin manager and its handles.

use thiserror::Error;

use crate::registry::types::PluginId;
use crate::transport::error::TransportError;

/// Failure while spawning, acquiring, or running a plugin instance.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ManagerError {
    /// No registered plugin (or running instance) has this id.
    #[error("Plugin not found: {0}")]
    PluginNotFound(PluginId),

    /// The instance's lifecycle state does not allow the operation; the string explains why.
    #[error("Plugin '{0}' is in an invalid state for this operation: {1}")]
    InvalidState(PluginId, String),

    /// The plugin's host process could not be started; the string is the OS error.
    #[error("Failed to spawn plugin process for '{0}': {1}")]
    SpawnFailed(PluginId, String),

    /// A health probe against the plugin failed; the string describes the failure.
    #[error("Health check failed for plugin '{0}': {1}")]
    HealthCheckFailed(PluginId, String),

    /// Task execution failed on the daemon side; the string describes the failure.
    #[error("Execution failed for plugin '{0}': {1}")]
    ExecutionFailed(PluginId, String),

    /// The plugin did not finish the task before its deadline.
    #[error("plugin '{0}' timed out")]
    Timeout(PluginId),

    /// The plugin process exited or disconnected mid-task.
    #[error("plugin '{0}' crashed during task")]
    Crashed(PluginId),

    /// The plugin itself reported a task error; the string is its message.
    #[error("plugin '{0}' reported an error: {1}")]
    Reported(PluginId, String),

    /// The instance is already executing another task.
    #[error("Plugin '{0}' is busy executing another task")]
    PluginBusy(PluginId),

    /// An IPC or gRPC transport operation failed.
    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    /// A filesystem or process I/O operation failed.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The plugin uses the scoped lifecycle, which is not supported yet.
    #[error("Scoped plugins are not yet implemented")]
    ScopedNotImplemented,

    /// The IPC reactor thread is unavailable or rejected the command.
    #[error("IPC reactor error: {0}")]
    Reactor(String),
}

/// Result alias whose error is [`ManagerError`].
pub type Result<T> = std::result::Result<T, ManagerError>;
