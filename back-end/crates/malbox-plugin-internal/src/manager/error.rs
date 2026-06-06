use thiserror::Error;

use crate::registry::types::PluginId;
use crate::transport::error::TransportError;

#[derive(Debug, Error)]
pub enum ManagerError {
    #[error("Plugin not found: {0}")]
    PluginNotFound(PluginId),

    #[error("Plugin '{0}' is in an invalid state for this operation: {1}")]
    InvalidState(PluginId, String),

    #[error("Failed to spawn plugin process for '{0}': {1}")]
    SpawnFailed(PluginId, String),

    #[error("Health check failed for plugin '{0}': {1}")]
    HealthCheckFailed(PluginId, String),

    #[error("Execution failed for plugin '{0}': {1}")]
    ExecutionFailed(PluginId, String),

    #[error("Plugin '{0}' is busy executing another task")]
    PluginBusy(PluginId),

    #[error("Transport error: {0}")]
    Transport(#[from] TransportError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Scoped plugins are not yet implemented")]
    ScopedNotImplemented,

    #[error("IPC reactor error: {0}")]
    Reactor(String),
}

pub type Result<T> = std::result::Result<T, ManagerError>;
