//! Event receiver for IPC communication.

use crate::error::{Result, TransportError};
use crate::messages::events::{Event, IpcPayload};
use crate::traits::TransportReceiver;

use core::time::Duration;
use iceoryx2::port::listener::Listener;
use iceoryx2::port::subscriber::Subscriber;
use iceoryx2::prelude::*;

/// Event receiver.
///
/// Used to receive events and payloads over a named IPC channel.
pub struct EventReceiver {
    listener: Listener<iceoryx2::service::ipc_threadsafe::Service>,
    subscriber: Subscriber<iceoryx2::service::ipc_threadsafe::Service, IpcPayload, ()>,
}

impl EventReceiver {
    /// Create a new event receiver on the given service names.
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

        let listener = event_service
            .listener_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let payload_service = node
            .service_builder(
                &payload_service_name
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<IpcPayload>()
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let subscriber = payload_service
            .subscriber_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self {
            listener,
            subscriber,
        })
    }
}

impl TransportReceiver for EventReceiver {
    fn wait(&self, timeout: Duration) -> Result<Option<Event>> {
        if let Some(event_id) = self
            .listener
            .timed_wait_one(timeout)
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
            && let Some(sample) = self
                .subscriber
                .receive()
                .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            let ipc_payload = sample.payload();
            let event = Event::from_id_and_payload(event_id.as_value(), ipc_payload)?;
            return Ok(Some(event));
        }

        Ok(None)
    }

    fn wait_blocking(&self) -> Result<Event> {
        loop {
            if let Some(event_id) = self
                .listener
                .blocking_wait_one()
                .map_err(|e| TransportError::Ipc(Box::new(e)))?
                && let Some(sample) = self
                    .subscriber
                    .receive()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?
            {
                let ipc_payload = sample.payload();
                let event = Event::from_id_and_payload(event_id.as_value(), ipc_payload)?;
                return Ok(event);
            }
        }
    }

    fn try_recv(&self) -> Result<Option<Event>> {
        if let Some(event_id) = self
            .listener
            .try_wait_one()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
            && let Some(sample) = self
                .subscriber
                .receive()
                .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            let ipc_payload = sample.payload();
            let event = Event::from_id_and_payload(event_id.as_value(), ipc_payload)?;
            return Ok(Some(event));
        }

        Ok(None)
    }
}
