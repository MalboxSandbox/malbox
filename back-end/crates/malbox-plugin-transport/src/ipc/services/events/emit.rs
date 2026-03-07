//! Event emitter for IPC communication.

use crate::error::{Result, TransportError};
use crate::messages::events::{Event, Payload};
use crate::traits::TransportEmitter;

use iceoryx2::port::notifier::Notifier;
use iceoryx2::port::publisher::Publisher;
use iceoryx2::prelude::*;

/// Event emitter.
///
/// Used to broadcast events and their payloads over a named IPC channel.
/// Events are sent via the notifier, while payloads are sent via pub/sub.
pub struct EventEmitter {
    notifier: Notifier<iceoryx2::service::ipc_threadsafe::Service>,
    publisher: Publisher<iceoryx2::service::ipc_threadsafe::Service, Payload, ()>,
}

impl EventEmitter {
    /// Create a new event emitter on the given service names.
    ///
    /// Uses `open_or_create` so either side can initialize the channel first.
    pub fn new(
        node: &Node<iceoryx2::service::ipc_threadsafe::Service>,
        event_service_name: &str,
        payload_service_name: &str,
    ) -> Result<Self> {
        let event_service = node
            .service_builder(
                &event_service_name
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .event()
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let notifier = event_service
            .notifier_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let payload_service = node
            .service_builder(
                &payload_service_name
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<Payload>()
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let publisher = payload_service
            .publisher_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self {
            notifier,
            publisher,
        })
    }
}

impl TransportEmitter for EventEmitter {
    fn emit(&self, event: Event, payload: Payload) -> Result<()> {
        // Send payload first (via pub/sub)
        let sample = self
            .publisher
            .loan_uninit()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        sample
            .write_payload(payload)
            .send()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        // Then notify (via event)
        self.notifier
            .notify_with_custom_event_id(EventId::new(event.event_id()))
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(())
    }
}
