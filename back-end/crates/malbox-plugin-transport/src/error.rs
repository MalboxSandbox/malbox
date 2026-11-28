//! Error type shared by every transport backend.

use thiserror::Error;

/// Failure raised by an IPC or gRPC transport operation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransportError {
    /// Underlying iceoryx2 (or other IPC backend) call failed.
    #[error("Internal IPC Error: {0}")]
    Ipc(#[from] Box<dyn std::error::Error + Send + Sync>),

    /// The outgoing IPC buffer had no free slot for the message.
    #[error("IPC buffer full: {0}")]
    BufferFull(String),

    /// The peer is not reachable or the endpoint cannot carry this operation.
    #[error("Not connected: {0}")]
    NotConnected(String),

    /// gRPC-level failure described by a free-form message.
    #[cfg(feature = "grpc")]
    #[error("gRPC error: {0}")]
    Grpc(String),

    /// Remote gRPC call returned a non-OK status.
    #[cfg(feature = "grpc")]
    #[error("gRPC status: {0}")]
    GrpcStatus(#[from] tonic::Status),

    /// Establishing or using the gRPC channel failed.
    #[cfg(feature = "grpc")]
    #[error("gRPC transport error: {0}")]
    GrpcTransport(#[from] tonic::transport::Error),
}

/// Result alias whose error is [`TransportError`].
pub type Result<T> = std::result::Result<T, TransportError>;
