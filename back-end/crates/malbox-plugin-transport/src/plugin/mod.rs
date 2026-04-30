//! Plugin-side transport types.
//!
//! Contains stream type aliases, the gRPC emitter/receiver, and re-exports of
//! the tonic-generated service types used by guest plugins.

mod emitter;
mod receiver;
mod server;

pub use emitter::GrpcEmitter;
pub use receiver::GrpcReceiver;
pub use server::{
    FileChunkStream, GuestPluginService, GuestPluginServiceServer, LogEntryStream,
    ResultChunkStream, TaskResultStream,
};
