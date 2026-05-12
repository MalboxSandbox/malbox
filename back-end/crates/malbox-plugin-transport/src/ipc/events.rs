//! Per-plugin event pub/sub channel for lightweight signals.
//!
//! Service name: `malbox/events/plugin/{plugin_id}` (per-plugin)
//! Service name: `malbox/events/daemon` (shared, daemon -> all plugins)
//! Pattern: publish_subscribe::<[u8]>() with user_header::<EventHeader>()
//!
//! Events are ultra-lightweight signals. The payload is typically empty or
//! contains a minimal postcard-encoded struct (e.g., file path for ResultAvailable).
//! Safe overflow is enabled - oldest events are dropped if subscribers are slow.

use crate::error::{Result, TransportError};
use crate::messages::events::Event;
use crate::traits::TransportEmitter;

use super::headers::{EventHeader, EventKind};

use iceoryx2::port::publisher::Publisher;
use iceoryx2::port::subscriber::Subscriber;
use iceoryx2::prelude::*;

impl TryFrom<&EventHeader> for Event {
    type Error = u16;

    fn try_from(header: &EventHeader) -> std::result::Result<Self, Self::Error> {
        let id = header.associated_id;
        match EventKind::try_from(header.event_type) {
            Ok(EventKind::TaskCreated) => Ok(Event::TaskCreated { task_id: id }),
            Ok(EventKind::TaskStarting) => Ok(Event::TaskStarting { task_id: id }),
            Ok(EventKind::TaskCompleted) => Ok(Event::TaskCompleted { task_id: id }),
            Ok(EventKind::TaskFailed) => Ok(Event::TaskFailed { task_id: id }),
            Ok(EventKind::TaskCanceled) => Ok(Event::TaskCanceled { task_id: id }),
            Ok(EventKind::PluginStarted) => Ok(Event::PluginStarted { plugin_id: id }),
            Ok(EventKind::PluginStopped) => Ok(Event::PluginStopped { plugin_id: id }),
            Ok(EventKind::PluginResultAvailable) => Ok(Event::PluginResultAvailable {
                source: String::new(),
                result_name: String::new(),
            }),
            Ok(EventKind::SampleStarted) => Ok(Event::SampleStarted { sample_id: id }),
            Ok(EventKind::SampleStopped) => Ok(Event::SampleStopped { sample_id: id }),
            Ok(EventKind::DaemonShutdown) => Ok(Event::DaemonShutdown),
            Ok(EventKind::ConfigReloaded) => Ok(Event::ConfigReloaded),
            Err(v) => Err(v),
        }
    }
}

impl From<&Event> for EventHeader {
    fn from(event: &Event) -> Self {
        let (event_type, associated_id) = match event {
            Event::TaskCreated { task_id } => (EventKind::TaskCreated as u16, *task_id),
            Event::TaskStarting { task_id } => (EventKind::TaskStarting as u16, *task_id),
            Event::TaskCompleted { task_id } => (EventKind::TaskCompleted as u16, *task_id),
            Event::TaskFailed { task_id } => (EventKind::TaskFailed as u16, *task_id),
            Event::TaskCanceled { task_id } => (EventKind::TaskCanceled as u16, *task_id),
            Event::PluginStarted { plugin_id } => (EventKind::PluginStarted as u16, *plugin_id),
            Event::PluginStopped { plugin_id } => (EventKind::PluginStopped as u16, *plugin_id),
            Event::PluginResultAvailable { .. } => (EventKind::PluginResultAvailable as u16, 0),
            Event::SampleStarted { sample_id } => (EventKind::SampleStarted as u16, *sample_id),
            Event::SampleStopped { sample_id } => (EventKind::SampleStopped as u16, *sample_id),
            Event::SampleResultProduced { sample_id } => {
                (EventKind::SampleStarted as u16, *sample_id)
            }
            Event::DaemonShutdown => (EventKind::DaemonShutdown as u16, 0),
            Event::ConfigReloaded => (EventKind::ConfigReloaded as u16, 0),
        };
        EventHeader {
            event_type,
            source_plugin_id: 0,
            associated_id,
            _reserved: [0; 6],
        }
    }
}

type IpcServiceType = iceoryx2::service::ipc_threadsafe::Service;

const MAX_EVENT_PAYLOAD: usize = 512;
const MAX_PLUGINS: usize = 65;
const SUBSCRIBER_BUFFER_SIZE: usize = 32;

fn plugin_event_service_name(plugin_id: &str) -> String {
    format!("malbox/events/plugin/{plugin_id}")
}

const DAEMON_EVENT_SERVICE_NAME: &str = "malbox/events/daemon";

/// Publisher for a plugin's own event channel.
/// Each plugin owns exactly one of these.
pub struct PluginEventPublisher {
    publisher: Publisher<IpcServiceType, [u8], EventHeader>,
}

impl PluginEventPublisher {
    pub fn new(node: &Node<IpcServiceType>, plugin_id: &str) -> Result<Self> {
        let name = plugin_event_service_name(plugin_id);
        let service = node
            .service_builder(
                &name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<[u8]>()
            .user_header::<EventHeader>()
            .max_publishers(1)
            .max_subscribers(MAX_PLUGINS)
            .subscriber_max_buffer_size(SUBSCRIBER_BUFFER_SIZE)
            .enable_safe_overflow(true)
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let publisher = service
            .publisher_builder()
            .initial_max_slice_len(MAX_EVENT_PAYLOAD)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { publisher })
    }

    /// Emit a lightweight event with an optional small payload.
    pub fn emit(&self, header: &EventHeader, payload: &[u8]) -> Result<()> {
        let mut sample = self
            .publisher
            .loan_slice_uninit(payload.len())
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        *sample.user_header_mut() = *header;
        let sample = sample.write_from_fn(|i| payload[i]);
        sample
            .send()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;
        Ok(())
    }

    /// Emit a signal-only event (no payload).
    pub fn emit_signal(&self, header: &EventHeader) -> Result<()> {
        self.emit(header, &[])
    }
}

/// Subscriber to a specific plugin's event channel.
/// Used by daemon and chaining plugins.
pub struct PluginEventSubscriber {
    subscriber: Subscriber<IpcServiceType, [u8], EventHeader>,
}

impl PluginEventSubscriber {
    pub fn new(node: &Node<IpcServiceType>, plugin_id: &str) -> Result<Self> {
        let name = plugin_event_service_name(plugin_id);
        let service = node
            .service_builder(
                &name
                    .as_str()
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<[u8]>()
            .user_header::<EventHeader>()
            .max_publishers(1)
            .max_subscribers(MAX_PLUGINS)
            .subscriber_max_buffer_size(SUBSCRIBER_BUFFER_SIZE)
            .enable_safe_overflow(true)
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let subscriber = service
            .subscriber_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { subscriber })
    }

    /// Non-blocking receive.
    pub fn try_recv(&self) -> Result<Option<ReceivedEvent>> {
        match self
            .subscriber
            .receive()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            Some(sample) => {
                let header = *sample.user_header();
                let payload = sample.payload().to_vec();
                Ok(Some(ReceivedEvent { header, payload }))
            }
            None => Ok(None),
        }
    }

    /// Drain all pending events.
    pub fn drain(&self) -> Result<Vec<ReceivedEvent>> {
        let mut events = Vec::new();
        while let Some(ev) = self.try_recv()? {
            events.push(ev);
        }
        Ok(events)
    }
}

/// Publisher for the shared daemon event channel.
/// Only the daemon creates one of these.
pub struct DaemonEventPublisher {
    publisher: Publisher<IpcServiceType, [u8], EventHeader>,
}

impl DaemonEventPublisher {
    pub fn new(node: &Node<IpcServiceType>) -> Result<Self> {
        let service = node
            .service_builder(
                &DAEMON_EVENT_SERVICE_NAME
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<[u8]>()
            .user_header::<EventHeader>()
            .max_publishers(1)
            .max_subscribers(MAX_PLUGINS)
            .subscriber_max_buffer_size(SUBSCRIBER_BUFFER_SIZE)
            .enable_safe_overflow(true)
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let publisher = service
            .publisher_builder()
            .initial_max_slice_len(MAX_EVENT_PAYLOAD)
            .allocation_strategy(AllocationStrategy::PowerOfTwo)
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { publisher })
    }

    /// Broadcast a daemon event to all plugins with a specific header and payload.
    pub fn emit_with_header(&self, header: &EventHeader, payload: &[u8]) -> Result<()> {
        let mut sample = self
            .publisher
            .loan_slice_uninit(payload.len())
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        *sample.user_header_mut() = *header;
        let sample = sample.write_from_fn(|i| payload[i]);
        sample
            .send()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;
        Ok(())
    }

    /// Broadcast a signal-only daemon event (no payload).
    pub fn emit_signal(&self, header: &EventHeader) -> Result<()> {
        self.emit_with_header(header, &[])
    }
}

impl TransportEmitter for DaemonEventPublisher {
    fn emit(&self, event: Event) -> Result<()> {
        self.emit_signal(&EventHeader::from(&event))
    }
}

/// Subscriber to the shared daemon event channel.
/// Every plugin creates one of these.
pub struct DaemonEventSubscriber {
    subscriber: Subscriber<IpcServiceType, [u8], EventHeader>,
}

impl DaemonEventSubscriber {
    pub fn new(node: &Node<IpcServiceType>) -> Result<Self> {
        let service = node
            .service_builder(
                &DAEMON_EVENT_SERVICE_NAME
                    .try_into()
                    .map_err(|e| TransportError::Ipc(Box::new(e)))?,
            )
            .publish_subscribe::<[u8]>()
            .user_header::<EventHeader>()
            .max_publishers(1)
            .max_subscribers(MAX_PLUGINS)
            .subscriber_max_buffer_size(SUBSCRIBER_BUFFER_SIZE)
            .enable_safe_overflow(true)
            .open_or_create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        let subscriber = service
            .subscriber_builder()
            .create()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?;

        Ok(Self { subscriber })
    }

    /// Non-blocking receive.
    pub fn try_recv(&self) -> Result<Option<ReceivedEvent>> {
        match self
            .subscriber
            .receive()
            .map_err(|e| TransportError::Ipc(Box::new(e)))?
        {
            Some(sample) => {
                let header = *sample.user_header();
                let payload = sample.payload().to_vec();
                Ok(Some(ReceivedEvent { header, payload }))
            }
            None => Ok(None),
        }
    }

    /// Drain all pending daemon events.
    pub fn drain(&self) -> Result<Vec<ReceivedEvent>> {
        let mut events = Vec::new();
        while let Some(ev) = self.try_recv()? {
            events.push(ev);
        }
        Ok(events)
    }
}

/// A received event with header and optional payload.
pub struct ReceivedEvent {
    pub header: EventHeader,
    pub payload: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_event_roundtrip() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let publisher = PluginEventPublisher::new(&node, "test-ev").expect("pub");
        let subscriber = PluginEventSubscriber::new(&node, "test-ev").expect("sub");

        let header = EventHeader {
            event_type: EventKind::PluginResultAvailable as u16,
            source_plugin_id: 5,
            associated_id: 42,
            _reserved: [0; 6],
        };

        publisher.emit_signal(&header).expect("emit");

        let ev = subscriber.try_recv().expect("recv").expect("event");
        assert_eq!(
            ev.header.event_type,
            EventKind::PluginResultAvailable as u16
        );
        assert_eq!(ev.header.source_plugin_id, 5);
        assert_eq!(ev.header.associated_id, 42);
        assert!(ev.payload.is_empty());
    }

    #[test]
    fn plugin_event_with_payload() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let publisher = PluginEventPublisher::new(&node, "test-ev-pl").expect("pub");
        let subscriber = PluginEventSubscriber::new(&node, "test-ev-pl").expect("sub");

        let header = EventHeader {
            event_type: EventKind::PluginResultAvailable as u16,
            source_plugin_id: 1,
            associated_id: 7,
            _reserved: [0; 6],
        };
        let path = b"/tmp/results/output.json";

        publisher.emit(&header, path).expect("emit");

        let ev = subscriber.try_recv().expect("recv").expect("event");
        assert_eq!(ev.payload, path);
    }

    #[test]
    fn daemon_event_broadcast() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let publisher = DaemonEventPublisher::new(&node).expect("pub");
        let sub1 = DaemonEventSubscriber::new(&node).expect("sub1");
        let sub2 = DaemonEventSubscriber::new(&node).expect("sub2");

        let header = EventHeader {
            event_type: EventKind::DaemonShutdown as u16,
            source_plugin_id: 0,
            associated_id: 0,
            _reserved: [0; 6],
        };

        publisher.emit_signal(&header).expect("emit");

        let ev1 = sub1.try_recv().expect("recv1").expect("event1");
        let ev2 = sub2.try_recv().expect("recv2").expect("event2");
        assert_eq!(ev1.header.event_type, EventKind::DaemonShutdown as u16);
        assert_eq!(ev2.header.event_type, EventKind::DaemonShutdown as u16);
    }

    #[test]
    fn drain_events() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let publisher = PluginEventPublisher::new(&node, "test-drain-ev").expect("pub");
        let subscriber = PluginEventSubscriber::new(&node, "test-drain-ev").expect("sub");

        for i in 0..5i32 {
            let header = EventHeader {
                event_type: EventKind::TaskCompleted as u16,
                source_plugin_id: 1,
                associated_id: i,
                _reserved: [0; 6],
            };
            publisher.emit_signal(&header).expect("emit");
        }

        let events = subscriber.drain().expect("drain");
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].header.associated_id, 0);
        assert_eq!(events[4].header.associated_id, 4);
    }

    #[test]
    fn safe_overflow_drops_oldest() {
        let node = NodeBuilder::new().create::<IpcServiceType>().expect("node");

        let publisher = PluginEventPublisher::new(&node, "test-overflow").expect("pub");
        let subscriber = PluginEventSubscriber::new(&node, "test-overflow").expect("sub");

        // Publish more events than the buffer can hold (buffer = 32)
        for i in 0..64i32 {
            let header = EventHeader {
                event_type: EventKind::TaskCreated as u16,
                source_plugin_id: 0,
                associated_id: i,
                _reserved: [0; 6],
            };
            publisher.emit_signal(&header).expect("emit");
        }

        let events = subscriber.drain().expect("drain");
        // Should have at most SUBSCRIBER_BUFFER_SIZE events (32), oldest dropped
        assert!(events.len() <= SUBSCRIBER_BUFFER_SIZE);
        // The remaining events should be the newest ones
        if !events.is_empty() {
            let last = events.last().unwrap();
            assert_eq!(last.header.associated_id, 63);
        }
    }
}
