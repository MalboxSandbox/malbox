//! Unified transport trait definitions.
//!
//! These traits abstract over the underlying transport mechanism (IPC, gRPC, etc.)
//! allowing the rest of the system to be transport-agnostic.

use core::time::Duration;

use crate::error::Result;
use crate::messages::events::Event;

/// Trait for emitting events to the other side of the transport.
///
/// Implemented by backend-specific emitters (e.g., `IpcEmitter`, `GrpcEmitter`).
/// Used by the daemon to broadcast events to plugins.
pub trait TransportEmitter {
    /// Emit an event.
    ///
    /// The implementation is responsible for serializing the event and
    /// delivering it to the other side.
    fn emit(&self, event: Event) -> Result<()>;
}

/// Trait for receiving events from the other side of the transport.
///
/// Implemented by backend-specific receivers (e.g., `IpcReceiver`, `GrpcReceiver`).
/// Used by plugins to receive events from the daemon.
pub trait TransportReceiver {
    /// Wait for an event with a timeout.
    ///
    /// Returns the event if one is received within the timeout,
    /// or `None` if the timeout expires.
    fn wait(&self, timeout: Duration) -> Result<Option<Event>>;

    /// Block until an event is received.
    ///
    /// Returns the event when one arrives.
    fn wait_blocking(&self) -> Result<Event>;

    /// Try to receive an event without blocking.
    ///
    /// Returns immediately with the event if available,
    /// or `None` if no event is pending.
    fn try_recv(&self) -> Result<Option<Event>>;
}

/// No-op emitter implementation for unit type.
///
/// This allows `PluginType` associated types to use `()` for sides
/// that are not needed (e.g., Guest plugins that only receive).
impl TransportEmitter for () {
    fn emit(&self, _event: Event) -> Result<()> {
        Ok(())
    }
}

/// No-op receiver implementation for unit type.
///
/// This allows `PluginType` associated types to use `()` for sides
/// that are not needed (e.g., Host plugins that only emit).
impl TransportReceiver for () {
    fn wait(&self, _timeout: Duration) -> Result<Option<Event>> {
        Ok(None)
    }

    fn wait_blocking(&self) -> Result<Event> {
        loop {
            std::thread::park();
        }
    }

    fn try_recv(&self) -> Result<Option<Event>> {
        Ok(None)
    }
}
