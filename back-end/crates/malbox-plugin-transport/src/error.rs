use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Internal IPC Error: {0}")]
    Ipc(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("IPC buffer full: {0}")]
    BufferFull(String),

    #[error("Not connected: {0}")]
    NotConnected(String),

    #[cfg(feature = "grpc")]
    #[error("gRPC error: {0}")]
    Grpc(String),

    #[cfg(feature = "grpc")]
    #[error("gRPC status: {0}")]
    GrpcStatus(#[from] tonic::Status),

    #[cfg(feature = "grpc")]
    #[error("gRPC transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),
}

pub type Result<T> = std::result::Result<T, TransportError>;
