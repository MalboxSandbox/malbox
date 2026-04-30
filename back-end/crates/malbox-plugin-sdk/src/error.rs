//! Error types for the plugin SDK.

use thiserror::Error;

/// Errors that can occur within the plugin SDK or be propagated from plugins.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SdkError {
    /// An error originating from the IPC or gRPC transport layer.
    #[error("transport error: {0}")]
    Transport(#[from] malbox_plugin_transport::error::TransportError),

    /// A failure during plugin or runtime initialization.
    #[error("initialization error: {0}")]
    Init(String),

    /// A blocking_send / try_send on a result or event channel failed.
    /// Almost always means the receiver was dropped (daemon disconnected).
    #[error("channel send failed: {0}")]
    Channel(String),

    /// An operation timed out waiting for an external signal.
    #[error("{operation} timed out after {elapsed:?}")]
    Timeout {
        operation: &'static str,
        elapsed: std::time::Duration,
    },

    /// A `Context` method was called from a place where the required
    /// state isn't set up (e.g. `push_result` outside `on_task`,
    /// `wait_for_execution` in a lifecycle handler).
    #[error("invalid context: {0}")]
    InvalidContext(&'static str),

    /// A JSON serialization or deserialization error (e.g. config parsing).
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// A standard I/O error (e.g. reading a sample file).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// An opaque error returned by plugin handler code.
    #[error("plugin error: {0}")]
    Plugin(Box<dyn std::error::Error + Send + Sync>),
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
    fn plugin_variant_wraps_custom_error() {
        #[derive(Debug)]
        struct MyErr(&'static str);
        impl std::fmt::Display for MyErr {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
        impl std::error::Error for MyErr {}

        let err: SdkError = SdkError::Plugin(Box::new(MyErr("custom boom")));
        assert!(err.to_string().contains("custom boom"));
    }

    #[test]
    fn serde_error_converts_to_sdk_error() {
        let json_err = serde_json::from_str::<String>("not json").unwrap_err();
        let sdk_err: SdkError = json_err.into();
        assert!(matches!(sdk_err, SdkError::Serialization(_)));
    }

    use std::time::Duration;

    #[test]
    fn channel_variant_formats_correctly() {
        let err = SdkError::Channel("push_result: receiver dropped".to_string());
        assert!(err.to_string().contains("channel send failed"));
        assert!(err.to_string().contains("push_result"));
    }

    #[test]
    fn timeout_variant_formats_correctly() {
        let err = SdkError::Timeout {
            operation: "some_operation",
            elapsed: Duration::from_secs(5),
        };
        let s = err.to_string();
        assert!(s.contains("some_operation"));
        assert!(s.contains("timed out"));
    }

    #[test]
    fn invalid_context_variant_formats_correctly() {
        let err = SdkError::InvalidContext("push_result called outside on_task");
        assert!(err.to_string().contains("invalid context"));
        assert!(err.to_string().contains("push_result"));
    }
}
