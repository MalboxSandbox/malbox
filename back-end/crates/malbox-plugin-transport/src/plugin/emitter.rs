//! gRPC-based event emitter for guest plugins.

use crate::error::{Result, TransportError};
use crate::grpc::{conversions, proto};
use crate::messages::events::{Event, Payload};
use crate::traits::TransportEmitter;

/// gRPC-based event emitter for guest plugins.
///
/// Two modes:
/// - **Task mode** (`with_task`): wraps a `result_tx` channel from `ExecuteTask` RPC.
///   `emit()` converts the event/payload to a `proto::TaskResult` and sends it through
///   the gRPC streaming response.
/// - **Noop mode** (`noop`): no active task stream. `emit()` is a silent no-op, used
///   during lifecycle callbacks (on_start, on_stop) that have no streaming channel.
pub struct GrpcEmitter {
    inner: GrpcEmitterInner,
}

enum GrpcEmitterInner {
    Task {
        task_id: i32,
        result_tx: tokio::sync::mpsc::Sender<std::result::Result<proto::TaskResult, tonic::Status>>,
    },
    Noop,
}

impl GrpcEmitter {
    /// Create an emitter that sends results through the gRPC streaming channel.
    pub fn with_task(
        task_id: i32,
        result_tx: tokio::sync::mpsc::Sender<std::result::Result<proto::TaskResult, tonic::Status>>,
    ) -> Self {
        Self {
            inner: GrpcEmitterInner::Task { task_id, result_tx },
        }
    }

    /// Create a no-op emitter for lifecycle callbacks without a task stream.
    pub fn noop() -> Self {
        Self {
            inner: GrpcEmitterInner::Noop,
        }
    }
}

impl GrpcEmitter {
    /// Send a raw `proto::TaskResult` through the gRPC streaming channel.
    ///
    /// This is used by the guest runtime to stream `PluginResult` items back
    /// to the daemon after converting them to proto form. No-ops in Noop mode.
    ///
    /// Must be called from a blocking context (uses `blocking_send`).
    pub fn send_task_result(&self, result: proto::TaskResult) -> Result<()> {
        match &self.inner {
            GrpcEmitterInner::Task { result_tx, .. } => result_tx
                .blocking_send(Ok(result))
                .map_err(|e| TransportError::Grpc(format!("channel send failed: {}", e))),
            GrpcEmitterInner::Noop => Ok(()),
        }
    }
}

impl TransportEmitter for GrpcEmitter {
    fn emit(&self, event: Event, payload: Payload) -> Result<()> {
        match &self.inner {
            GrpcEmitterInner::Task { task_id, result_tx } => {
                let task_result = conversions::event_to_task_result(*task_id, &event, &payload);
                result_tx
                    .blocking_send(Ok(task_result))
                    .map_err(|e| TransportError::Grpc(format!("channel send failed: {}", e)))
            }
            GrpcEmitterInner::Noop => Ok(()),
        }
    }
}
