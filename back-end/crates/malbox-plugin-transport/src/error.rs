use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("Internal IPC Error: {0}")]
    Ipc(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("gRPC error: {0}")]
    Grpc(String),

    #[error("gRPC status: {0}")]
    GrpcStatus(#[from] tonic::Status),

    #[error("gRPC transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),

    #[error("Invalid event ID: {0}")]
    InvalidEventId(usize),
}

pub type Result<T> = std::result::Result<T, TransportError>;
