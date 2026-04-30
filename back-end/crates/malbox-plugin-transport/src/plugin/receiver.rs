//! gRPC-based event receiver (stub).
//!
//! The reactive gRPC model doesn't use polling — the daemon calls RPCs directly
//! and the server handler is invoked. This exists only to satisfy associated type
//! bounds in the transport abstraction.

use crate::error::{Result, TransportError};
use crate::messages::events::Event;
use crate::traits::TransportReceiver;
use core::time::Duration;

pub struct GrpcReceiver {
    _private: (),
}

impl Default for GrpcReceiver {
    fn default() -> Self {
        Self::new()
    }
}

impl GrpcReceiver {
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl TransportReceiver for GrpcReceiver {
    fn wait(&self, _timeout: Duration) -> Result<Option<Event>> {
        Ok(None)
    }

    fn wait_blocking(&self) -> Result<Event> {
        Err(TransportError::Grpc(
            "GrpcReceiver does not support polling; use the GuestPluginService model".into(),
        ))
    }

    fn try_recv(&self) -> Result<Option<Event>> {
        Ok(None)
    }
}
