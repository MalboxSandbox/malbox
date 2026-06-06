//! Command-channel wakeup for the daemon-side IPC reactor.
//!
//! Service name: `malbox/daemon/reactor-wakeup`.
//! The notifier side is shared by many tokio tasks, so it uses the
//! `ipc_threadsafe` variant - the one place the mutex-protected policy
//! survives, on a cold path where its cost is irrelevant. The listener side
//! lives on the reactor thread and uses the single-threaded variant. The two
//! variants share identical shared-memory primitives, so they interoperate
//! on the same service.

use crate::error::{Result, TransportError};

use iceoryx2::port::listener::Listener;
use iceoryx2::port::notifier::Notifier;
use iceoryx2::prelude::*;

use super::{IpcService, IpcServiceThreadsafe};

const REACTOR_WAKEUP_SERVICE_NAME: &str = "malbox/daemon/reactor-wakeup";
const MAX_NOTIFIERS: usize = 2;
const MAX_LISTENERS: usize = 2;

/// Wakes the reactor thread from async tasks. `Send + Sync`; share via `Arc`.
pub struct ReactorWaker {
    /// Keeps the threadsafe node (and thus the notifier) alive.
    _node: Node<IpcServiceThreadsafe>,
    notifier: Notifier<IpcServiceThreadsafe>,
}

impl ReactorWaker {
    pub fn new() -> Result<Self> {
        let node = NodeBuilder::new()
            .signal_handling_mode(SignalHandlingMode::Disabled)
            .create::<IpcServiceThreadsafe>()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let service = node
            .service_builder(
                &REACTOR_WAKEUP_SERVICE_NAME
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

        Ok(Self {
            _node: node,
            notifier,
        })
    }

    /// Wake the reactor so it drains its command queue.
    pub fn wake(&self) -> Result<()> {
        self.notifier
            .notify()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;
        Ok(())
    }
}

/// Reactor-side listener for command wakeups.
pub struct ReactorWakeupListener {
    listener: Listener<IpcService>,
}

impl ReactorWakeupListener {
    pub fn new(node: &Node<IpcService>) -> Result<Self> {
        let service = node
            .service_builder(
                &REACTOR_WAKEUP_SERVICE_NAME
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

    /// Non-blocking drain of all pending wakeups (the payload carries no
    /// information; commands live in the mpsc queue).
    pub fn drain(&self) -> Result<()> {
        while self
            .listener
            .try_wait_one()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
            .is_some()
        {}
        Ok(())
    }
}
