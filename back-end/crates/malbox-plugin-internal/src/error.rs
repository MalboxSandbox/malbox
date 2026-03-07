use thiserror::Error;

#[derive(Debug, Error)]
pub enum InternalPluginError {
    #[error("Internal Transport Error: {0}")]
    Transport(#[from] crate::transport::error::TransportError),
}

pub type Result<T> = std::result::Result<T, InternalPluginError>;
