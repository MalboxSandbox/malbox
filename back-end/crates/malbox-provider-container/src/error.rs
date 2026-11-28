use std::time::Duration;
use thiserror::Error;

/// Error type for container provider operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContainerError {
    /// Error returned by the container engine API.
    #[error("Container engine error: {0}")]
    Docker(#[from] bollard::errors::Error),
    /// Invalid provider or machine configuration.
    #[error("Configuration error: {0}")]
    Config(String),
    /// Operation or platform not supported by this provider.
    #[error("Unsupported: {0}")]
    Unsupported(String),
    /// A container, image or snapshot does not exist.
    #[error("Not found: {0}")]
    NotFound(String),
    /// Image archive could not be loaded.
    #[error("Image error: {0}")]
    Image(String),
    /// Host I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// JSON (de)serialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    /// Timeout error.
    #[error("Operation `{operation}` timed out after {duration:?}")]
    Timeout {
        /// Name of the operation that timed out.
        operation: String,
        /// How long the operation was allowed to run.
        duration: Duration,
    },
}

/// HTTP status code of an engine error response, if it is one.
pub(crate) fn status(e: &bollard::errors::Error) -> Option<u16> {
    match e {
        bollard::errors::Error::DockerResponseServerError { status_code, .. } => Some(*status_code),
        _ => None,
    }
}

/// Map engine responses with one of `codes` to `Ok(None)`.
pub(crate) fn tolerate<T>(
    result: Result<T, bollard::errors::Error>,
    codes: &[u16],
) -> Result<Option<T>, ContainerError> {
    match result {
        Ok(v) => Ok(Some(v)),
        Err(e) if status(&e).is_some_and(|s| codes.contains(&s)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}
