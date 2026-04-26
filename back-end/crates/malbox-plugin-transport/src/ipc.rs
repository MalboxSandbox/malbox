//! IPC transport layer using iceoryx2 for host plugins communication.

pub mod services;

pub use iceoryx2::node::Node;
pub use iceoryx2::prelude::NodeBuilder;
pub use iceoryx2::service::ipc_threadsafe::Service as IpcService;
pub use services::events::{EventEmitter, EventReceiver, daemon_channel, plugin_channel};
pub use services::tasks;
