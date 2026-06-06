//! IPC transport layer using iceoryx2 for host plugins communication.
//!
//! `IpcService` (single-threaded, Rc-based ports) is the default for every
//! port; ports are confined to one thread on each side. `IpcServiceThreadsafe`
//! exists solely for the reactor waker, whose notifier is shared by many
//! async tasks. The two variants interoperate on the same services.

pub mod daemon_notify;
pub mod events;
pub mod headers;
pub mod notify;
pub mod reactor_wakeup;
pub mod results;
pub mod tasks;

pub use iceoryx2::node::Node;
pub use iceoryx2::prelude::NodeBuilder;
pub use iceoryx2::prelude::SignalHandlingMode;
pub use iceoryx2::prelude::{CallbackProgression, WaitSetBuilder};
pub use iceoryx2::service::ipc::Service as IpcService;
pub use iceoryx2::service::ipc_threadsafe::Service as IpcServiceThreadsafe;

pub use daemon_notify::{DaemonNotifier, DaemonNotifyKind, DaemonNotifyListener};
pub use events::{
    DaemonEventPublisher, DaemonEventSubscriber, PluginEventPublisher, PluginEventSubscriber,
    ReceivedEvent,
};
pub use headers::{EventHeader, EventKind, ResultHeader, TaskRequestHeader, TaskResponseHeader};
pub use notify::{NotifyKind, PluginNotifier, PluginWakeupListener};
pub use reactor_wakeup::{ReactorWaker, ReactorWakeupListener};
pub use results::{ResultPublisher, ResultSubscriber};
pub use tasks::{ActiveTaskRequest, TaskClient, TaskPendingResponse, TaskResponse, TaskServer};

/// Serializes tests using the shared `malbox/daemon/notify` service. With no
/// long-lived port pinning it in the test binary (production has the
/// reactor's listener), parallel open/teardown can race iceoryx2's unretried
/// `IsMarkedForDestruction` window. Declare the guard first so it drops last.
#[cfg(test)]
pub(crate) fn shared_notify_service_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
