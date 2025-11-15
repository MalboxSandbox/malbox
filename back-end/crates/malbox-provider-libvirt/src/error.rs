use std::time::Duration;
use thiserror::Error;

/// Main error type for libvirt provider operations.
#[derive(Debug, Error)]
pub enum LibvirtError {
    /// Error from libvirt library itself.
    #[error("Libvirt error: {0}")]
    Libvirt(String),
    /// Timeout error.
    #[error("Operation `{operation}` timed out after {duration:?}")]
    Timeout {
        operation: String,
        duration: Duration,
    },
    /// JSON serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Automatic conversion from libvirt errors.
impl From<virt::error::Error> for LibvirtError {
    fn from(e: virt::error::Error) -> Self {
        LibvirtError::Libvirt(e.to_string())
    }
}
