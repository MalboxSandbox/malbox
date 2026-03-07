//! Unified transport trait definitions.
//!
//! These traits abstract over the underlying transport mechanism (IPC, gRPC, etc.)
//! allowing the rest of the system to be transport-agnostic.

use core::time::Duration;

use crate::error::Result;
use crate::messages::events::{Event, Payload};

/// Trait for emitting events to the other side of the transport.
///
/// Implemented by backend-specific emitters (e.g., `IpcEmitter`, `GrpcEmitter`).
/// Used by the daemon to broadcast events and payloads to plugins.
pub trait TransportEmitter {
    /// Emit an event with its associated payload.
    ///
    /// The implementation is responsible for ensuring the payload is delivered
    /// before or atomically with the event signal.
    fn emit(&self, event: Event, payload: Payload) -> Result<()>;
}

/// Trait for receiving events from the other side of the transport.
///
/// Implemented by backend-specific receivers (e.g., `IpcReceiver`, `GrpcReceiver`).
/// Used by plugins to receive events and payloads from the daemon.
pub trait TransportReceiver {
    /// Wait for an event with a timeout.
    ///
    /// Returns the event and its payload if one is received within the timeout,
    /// or `None` if the timeout expires.
    fn wait(&self, timeout: Duration) -> Result<Option<(Event, Payload)>>;

    /// Block until an event is received.
    ///
    /// Returns the event and its payload when one arrives.
    fn wait_blocking(&self) -> Result<(Event, Payload)>;

    /// Try to receive an event without blocking.
    ///
    /// Returns immediately with the event and payload if available,
    /// or `None` if no event is pending.
    fn try_recv(&self) -> Result<Option<(Event, Payload)>>;
}

/// No-op emitter implementation for unit type.
///
/// This allows `PluginType` associated types to use `()` for sides
/// that are not needed (e.g., Guest plugins that only receive).
impl TransportEmitter for () {
    fn emit(&self, _event: Event, _payload: Payload) -> Result<()> {
        Ok(())
    }
}

/// No-op receiver implementation for unit type.
///
/// This allows `PluginType` associated types to use `()` for sides
/// that are not needed (e.g., Host plugins that only emit).
impl TransportReceiver for () {
    fn wait(&self, _timeout: Duration) -> Result<Option<(Event, Payload)>> {
        Ok(None)
    }

    fn wait_blocking(&self) -> Result<(Event, Payload)> {
        loop {
            std::thread::park();
        }
    }

    fn try_recv(&self) -> Result<Option<(Event, Payload)>> {
        Ok(None)
    }
}
