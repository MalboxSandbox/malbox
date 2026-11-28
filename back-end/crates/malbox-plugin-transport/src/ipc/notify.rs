//! `WaitSet` notification service for per-plugin wakeup.
//!
//! Each plugin gets one shared event service (`malbox/plugin/{id}/notify`).
//! Different senders use different `EventId`s to signal which channel has data:
//! - `EventId(0)` = task request available
//! - `EventId(1)` = daemon event available

use crate::error::{Result, TransportError};

use iceoryx2::port::listener::Listener;
use iceoryx2::port::notifier::Notifier;
use iceoryx2::prelude::*;

type IpcServiceType = iceoryx2::service::ipc::Service;

/// Discriminates which channel triggered a wakeup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
#[non_exhaustive]
pub enum NotifyKind {
    /// A task request is waiting in the plugin's request channel.
    TaskAvailable = 0,
    /// A daemon broadcast event is waiting in the daemon event channel.
    DaemonEvent = 1,
}

impl NotifyKind {
    /// iceoryx2 event id that encodes this kind on the wire.
    pub fn event_id(self) -> EventId {
        EventId::new(match self {
            Self::TaskAvailable => 0,
            Self::DaemonEvent => 1,
        })
    }

    /// Decode a received event id; `None` if it names no known kind.
    pub fn from_event_id(id: &EventId) -> Option<Self> {
        match id.as_value() {
            0 => Some(Self::TaskAvailable),
            1 => Some(Self::DaemonEvent),
            _ => None,
        }
    }
}

fn notify_service_name(plugin_id: &str) -> String {
    format!("malbox/plugin/{plugin_id}/notify")
}

/// Sends wakeup notifications to a specific plugin's `WaitSet`.
#[derive(Debug)]
pub struct PluginNotifier {
    notifier: Notifier<IpcServiceType>,
}

impl PluginNotifier {
    /// Open (or create) the plugin's notify service and attach a notifier.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Ipc`] if the service name is invalid or the
    /// service or notifier cannot be created.
    pub fn new(node: &Node<IpcServiceType>, plugin_id: &str) -> Result<Self> {
        let name = notify_service_name(plugin_id);
        let service = node
            .service_builder(
                &name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .event()
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let notifier = service
            .notifier_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { notifier })
    }

    /// Wake up the plugin, indicating which channel has data.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Ipc`] if the notification cannot be delivered.
    pub fn wake(&self, kind: NotifyKind) -> Result<()> {
        self.notifier
            .notify_with_custom_event_id(kind.event_id())
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;
        Ok(())
    }
}

/// Listener side attached to the plugin's `WaitSet` for multiplexed wakeup.
#[derive(Debug)]
pub struct PluginWakeupListener {
    listener: Listener<IpcServiceType>,
}

impl PluginWakeupListener {
    /// Open (or create) the plugin's notify service and attach a listener.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Ipc`] if the service name is invalid or the
    /// service or listener cannot be created.
    pub fn new(node: &Node<IpcServiceType>, plugin_id: &str) -> Result<Self> {
        let name = notify_service_name(plugin_id);
        let service = node
            .service_builder(
                &name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .event()
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let listener = service
            .listener_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { listener })
    }

    /// Get a reference to the underlying listener for `WaitSet` attachment.
    pub fn listener(&self) -> &Listener<IpcServiceType> {
        &self.listener
    }

    /// Non-blocking drain of all pending notification kinds.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Ipc`] if polling the listener fails.
    pub fn drain(&self) -> Result<Vec<NotifyKind>> {
        let mut kinds = Vec::new();
        while let Some(id) = self
            .listener
            .try_wait_one()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            if let Some(kind) = NotifyKind::from_event_id(&id) {
                kinds.push(kind);
            }
        }
        Ok(kinds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_roundtrip() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let notifier = PluginNotifier::new(&node, "test-notify").expect("notifier");
        let listener = PluginWakeupListener::new(&node, "test-notify").expect("listener");

        notifier.wake(NotifyKind::TaskAvailable).expect("wake");
        notifier.wake(NotifyKind::DaemonEvent).expect("wake");

        let kinds = listener.drain().expect("drain");
        assert!(kinds.contains(&NotifyKind::TaskAvailable));
        assert!(kinds.contains(&NotifyKind::DaemonEvent));
    }

    #[test]
    fn drain_empty_returns_empty() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let listener = PluginWakeupListener::new(&node, "test-empty").expect("listener");
        let kinds = listener.drain().expect("drain");
        assert!(kinds.is_empty());
    }
}
