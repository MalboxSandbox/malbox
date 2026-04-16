//! Plugin-side transport types.
//!
//! Contains the gRPC server, handler trait, and emitter/receiver types used by
//! guest plugins to expose a service and communicate with the daemon.

mod emitter;
mod receiver;
mod server;

pub use emitter::GrpcEmitter;
pub use receiver::GrpcReceiver;
pub use server::{GrpcServer, GuestPluginHandler, LogEntryStream, ResultChunkStream};
