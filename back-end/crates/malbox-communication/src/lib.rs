//! Malbox communication library.
//!
//! This crate provides zero-copy IPC communication primitives for Malbox
//! using iceoryx2. It supports both host-side and plugin-side communication
//! with a generic, reusable architecture.

pub mod error;
pub mod ipc;
pub mod messages;

pub use error::{CommunicationError, Result};
pub use ipc::{Channel, ChannelConfig, ChannelRole, host::HostChannel, plugin::PluginChannel};
pub use messages::{
    ChannelMessage, CommandMessage, EventMessage, MessagePayload, MessageType, ResultMessage,
    TaskMessage,
};
