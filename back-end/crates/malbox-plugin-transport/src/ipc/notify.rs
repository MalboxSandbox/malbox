//! WaitSet notification service for per-plugin wakeup.
//!
//! Each plugin gets one shared event service (`malbox/plugin/{id}/notify`).
//! Different senders use different EventIds to signal which channel has data:
//! - EventId(0) = task request available
//! - EventId(1) = daemon event available
//! - EventId(2) = result from subscription available

use crate::error::{Result, TransportError};

use iceoryx2::port::listener::Listener;
use iceoryx2::port::notifier::Notifier;
use iceoryx2::prelude::*;

/// Discriminates which channel triggered a wakeup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum NotifyKind {
    TaskAvailable = 0,
    DaemonEvent = 1,
    ResultAvailable = 2,
}

impl NotifyKind {
    pub fn event_id(self) -> EventId {
        EventId::new(self as usize)
    }

    pub fn from_event_id(id: &EventId) -> Option<Self> {
        match id.as_value() {
            0 => Some(Self::TaskAvailable),
            1 => Some(Self::DaemonEvent),
            2 => Some(Self::ResultAvailable),
            _ => None,
        }
    }
}

fn notify_service_name(plugin_id: &str) -> String {
    format!("malbox/plugin/{plugin_id}/notify")
}

/// Sends wakeup notifications to a specific plugin's WaitSet.
pub struct PluginNotifier {
    notifier: Notifier<iceoryx2::service::ipc_threadsafe::Service>,
}

impl PluginNotifier {
    pub fn new(
        node: &Node<iceoryx2::service::ipc_threadsafe::Service>,
        plugin_id: &str,
    ) -> Result<Self> {
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
    pub fn wake(&self, kind: NotifyKind) -> Result<()> {
        self.notifier
            .notify_with_custom_event_id(kind.event_id())
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;
        Ok(())
    }
}

/// Listener side attached to the plugin's WaitSet for multiplexed wakeup.
pub struct PluginWakeupListener {
    listener: Listener<iceoryx2::service::ipc_threadsafe::Service>,
}

impl PluginWakeupListener {
    pub fn new(
        node: &Node<iceoryx2::service::ipc_threadsafe::Service>,
        plugin_id: &str,
    ) -> Result<Self> {
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

    /// Get a reference to the underlying listener for WaitSet attachment.
    pub fn listener(&self) -> &Listener<iceoryx2::service::ipc_threadsafe::Service> {
        &self.listener
    }

    /// Non-blocking drain of all pending notification kinds.
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
        let node = NodeBuilder::new()
            .create::<iceoryx2::service::ipc_threadsafe::Service>()
            .expect("node");

        let notifier = PluginNotifier::new(&node, "test-notify").expect("notifier");
        let listener = PluginWakeupListener::new(&node, "test-notify").expect("listener");

        notifier.wake(NotifyKind::TaskAvailable).expect("wake");
        notifier.wake(NotifyKind::DaemonEvent).expect("wake");
        notifier.wake(NotifyKind::ResultAvailable).expect("wake");

        let kinds = listener.drain().expect("drain");
        assert!(kinds.contains(&NotifyKind::TaskAvailable));
        assert!(kinds.contains(&NotifyKind::DaemonEvent));
        assert!(kinds.contains(&NotifyKind::ResultAvailable));
    }

    #[test]
    fn drain_empty_returns_empty() {
        let node = NodeBuilder::new()
            .create::<iceoryx2::service::ipc_threadsafe::Service>()
            .expect("node");

        let listener = PluginWakeupListener::new(&node, "test-empty").expect("listener");
        let kinds = listener.drain().expect("drain");
        assert!(kinds.is_empty());
    }
}
