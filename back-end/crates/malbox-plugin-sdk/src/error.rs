use thiserror::Error;

#[derive(Debug, Error)]
pub enum SdkError {
    #[error("Transport error: {0}")]
    Transport(#[from] malbox_plugin_transport::error::TransportError),

    #[error("IPC initialization error: {0}")]
    Init(String),

    #[error("Plugin handler error: {0}")]
    Handler(String),
}

pub type Result<T> = std::result::Result<T, SdkError>;
