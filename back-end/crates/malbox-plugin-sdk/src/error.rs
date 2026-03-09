//! Error types for the plugin SDK.

use thiserror::Error;

/// Errors that can occur within the plugin SDK or be propagated from plugins.
#[derive(Debug, Error)]
pub enum SdkError {
    /// An error originating from the IPC or gRPC transport layer.
    #[error("transport error: {0}")]
    Transport(#[from] malbox_plugin_transport::error::TransportError),

    /// A failure during plugin or runtime initialization.
    #[error("initialization error: {0}")]
    Init(String),

    /// A JSON serialization or deserialization error (e.g. config parsing).
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A standard I/O error (e.g. reading a sample file).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// An opaque error returned by plugin handler code via [`anyhow::Error`].
    #[error("{0}")]
    Plugin(#[from] anyhow::Error),
}

/// A [`Result`](std::result::Result) alias using [`SdkError`] as the error type.
pub type Result<T> = std::result::Result<T, SdkError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_error_converts_to_sdk_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let sdk_err: SdkError = io_err.into();
        assert!(matches!(sdk_err, SdkError::Io(_)));
    }

    #[test]
    fn anyhow_error_converts_to_sdk_error() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let sdk_err: SdkError = anyhow_err.into();
        assert!(matches!(sdk_err, SdkError::Plugin(_)));
        assert!(sdk_err.to_string().contains("something went wrong"));
    }

    #[test]
    fn serde_error_converts_to_sdk_error() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let sdk_err: SdkError = json_err.into();
        assert!(matches!(sdk_err, SdkError::Serialization(_)));
    }
}
