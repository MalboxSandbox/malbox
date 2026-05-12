//! IPC transport layer using iceoryx2 for host plugins communication.

pub mod events;
pub mod headers;
pub mod notify;
pub mod results;
pub mod tasks;

pub use iceoryx2::node::Node;
pub use iceoryx2::prelude::NodeBuilder;
pub use iceoryx2::prelude::{CallbackProgression, WaitSetBuilder};
pub use iceoryx2::service::ipc_threadsafe::Service as IpcService;

pub use events::{
    DaemonEventPublisher, DaemonEventSubscriber, PluginEventPublisher, PluginEventSubscriber,
    ReceivedEvent,
};
pub use headers::{EventHeader, EventKind, ResultHeader, TaskRequestHeader, TaskResponseHeader};
pub use notify::{NotifyKind, PluginNotifier, PluginWakeupListener};
pub use results::{ResultPublisher, ResultSubscriber};
pub use tasks::{ActiveTaskRequest, TaskClient, TaskPendingResponse, TaskResponse, TaskServer};
