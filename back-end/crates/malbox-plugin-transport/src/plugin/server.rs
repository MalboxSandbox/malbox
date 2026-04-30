//! Stream type aliases and re-exports for the guest plugin gRPC service.

use crate::grpc::proto;
use std::pin::Pin;
use tonic::Status;

pub use crate::grpc::proto::guest_plugin_service_server::{
    GuestPluginService, GuestPluginServiceServer,
};

/// The stream type returned from the `execute_task` RPC method.
pub type TaskResultStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<proto::TaskResult, Status>> + Send>>;

/// The stream type returned from the `pull_file` RPC method.
pub type FileChunkStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<proto::FileChunk, Status>> + Send>>;

/// The stream type returned from the `stream_logs` RPC method.
pub type LogEntryStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<proto::LogEntry, Status>> + Send>>;

/// The stream type returned from the `pull_result` RPC method.
pub type ResultChunkStream =
    Pin<Box<dyn tokio_stream::Stream<Item = Result<proto::ResultChunk, Status>> + Send>>;
