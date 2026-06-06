//! Shared plugin -> daemon notification service.
//!
//! Service name: `malbox/daemon/notify` (one service; every plugin notifies it).
//! The daemon-side IPC reactor attaches the listener to its WaitSet and wakes
//! whenever any plugin has produced data. EventIds carry a [`DaemonNotifyKind`].
//!
//! Notifying after every send is a mandatory part of the host plugin IPC
//! contract - the daemon has no polling fallback for plugins that never
//! notify. The notify calls live inside the plugin-side transport ports
//! (`TaskServer`, `PluginEventPublisher`), so SDK runtimes cannot forget them.

use crate::error::{Result, TransportError};

use iceoryx2::port::listener::Listener;
use iceoryx2::port::notifier::Notifier;
use iceoryx2::prelude::*;

use super::IpcService;

const DAEMON_NOTIFY_SERVICE_NAME: &str = "malbox/daemon/notify";
// Each plugin process opens two notifiers (TaskServer + PluginEventPublisher),
// so the budget is 2 x 65 plugins, plus headroom.
const MAX_NOTIFIERS: usize = 140;
const MAX_LISTENERS: usize = 2;

/// Which kind of data a plugin produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum DaemonNotifyKind {
    /// A task response (streamed result chunk, progress, or final marker).
    TaskResponse = 0,
    /// A plugin event emission (e.g. PluginStarted, TaskCompleted).
    PluginEvent = 1,
    /// A result published on the plugin's result channel (chaining).
    /// Defined for protocol completeness; nothing fires it yet.
    ResultPublished = 2,
}

impl DaemonNotifyKind {
    pub fn event_id(self) -> EventId {
        EventId::new(self as usize)
    }

    pub fn from_event_id(id: &EventId) -> Option<Self> {
        match id.as_value() {
            0 => Some(Self::TaskResponse),
            1 => Some(Self::PluginEvent),
            2 => Some(Self::ResultPublished),
            _ => None,
        }
    }
}

/// Plugin-side notifier on the shared daemon notification service.
pub struct DaemonNotifier {
    notifier: Notifier<IpcService>,
}

impl DaemonNotifier {
    pub fn new(node: &Node<IpcService>) -> Result<Self> {
        let service = node
            .service_builder(
                &DAEMON_NOTIFY_SERVICE_NAME
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .event()
            .max_notifiers(MAX_NOTIFIERS)
            .max_listeners(MAX_LISTENERS)
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let notifier = service
            .notifier_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { notifier })
    }

    /// Tell the daemon this plugin produced data of the given kind.
    ///
    /// Failures may be ignored: the data send has already committed by the
    /// time this runs, so a lost notify costs at most a delayed wakeup
    /// (bounded by the daemon's interval tick), never data loss.
    pub fn notify(&self, kind: DaemonNotifyKind) -> Result<()> {
        self.notifier
            .notify_with_custom_event_id(kind.event_id())
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;
        Ok(())
    }
}

/// Daemon-side listener, attached to the reactor's WaitSet.
pub struct DaemonNotifyListener {
    listener: Listener<IpcService>,
}

impl DaemonNotifyListener {
    pub fn new(node: &Node<IpcService>) -> Result<Self> {
        let service = node
            .service_builder(
                &DAEMON_NOTIFY_SERVICE_NAME
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .event()
            .max_notifiers(MAX_NOTIFIERS)
            .max_listeners(MAX_LISTENERS)
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let listener = service
            .listener_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { listener })
    }

    /// Get a reference to the underlying listener for WaitSet attachment.
    pub fn listener(&self) -> &Listener<IpcService> {
        &self.listener
    }

    /// Non-blocking drain of all pending notification kinds.
    pub fn drain(&self) -> Result<Vec<DaemonNotifyKind>> {
        let mut kinds = Vec::new();
        while let Some(id) = self
            .listener
            .try_wait_one()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            if let Some(kind) = DaemonNotifyKind::from_event_id(&id) {
                kinds.push(kind);
            }
        }
        Ok(kinds)
    }
}
