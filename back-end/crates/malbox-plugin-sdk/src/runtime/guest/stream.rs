//! Result streaming + log entry conversion for the guest runtime.
//!
//! `on_execute_task` is the most complex RPC handler: it streams partial
//! results back over an mpsc channel as the plugin's `on_task` produces
//! them. This module exposes the streaming logic as a free function so
//! the bridge in [`super::bridge`] stays focused on RPC plumbing.

use crate::context::Context;
use crate::log::LogEntry;
use crate::plugin::Plugin;
use crate::stash::ResultStash;
use crate::types::Task;
use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::plugin::GrpcEmitter;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::Status;
use tracing::error;

use super::exec::ExecutionWaiter;

/// Run a plugin's `on_task` and stream results back over `result_tx`.
///
/// This is invoked from `tokio::task::spawn_blocking` so it can call into
/// the plugin's synchronous `on_task` method without blocking the runtime.
#[allow(clippy::too_many_arguments)]
pub(super) fn execute_task<P: Plugin>(
    plugin: Arc<P>,
    task_id: i32,
    sample_path: PathBuf,
    config: HashMap<String, String>,
    result_tx: mpsc::Sender<std::result::Result<proto::TaskResult, Status>>,
    waiter: ExecutionWaiter,
    stash: Arc<ResultStash>,
) {
    // Send READY signal to indicate the task handler is about to start.
    let ready_result = proto::TaskResult {
        task_id,
        result_name: String::new(),
        data: vec![],
        format: proto::ResultFormat::Unspecified.into(),
        is_final: false,
        kind: proto::ResultKind::Ready.into(),
    };
    if let Err(e) = result_tx.blocking_send(Ok(ready_result)) {
        error!("Failed to send READY signal: {}", e);
        return;
    }

    let emitter = GrpcEmitter::with_task(task_id, result_tx.clone());
    let mut ctx = Context::new(&emitter, Some(result_tx.clone()), Some(waiter))
        .with_task_id(task_id)
        .with_stash(Arc::clone(&stash));

    // Propagate the analysis timeout from config so wait_for_execution()
    // automatically uses the user-requested duration.
    if let Some(secs) = config
        .get("analysis_timeout")
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|&s| s > 0)
    {
        ctx = ctx.with_analysis_timeout(std::time::Duration::from_secs(secs));
    }

    let task = Task::new(task_id, sample_path, config);

    if let Err(e) = plugin.on_task(task, &ctx) {
        error!("Plugin task handler error: {}", e);
    }

    // Always send the final marker, regardless of success/failure.
    // Per Task 17 investigation: result_name MUST be empty (daemon guard at
    // handle.rs:164 silently drops empty-name results from outputs).
    let final_marker = proto::TaskResult {
        task_id,
        result_name: String::new(),
        data: vec![],
        format: proto::ResultFormat::Unspecified.into(),
        is_final: true,
        kind: proto::ResultKind::Result.into(),
    };
    if let Err(e) = emitter.send_task_result(final_marker) {
        error!("Failed to send final marker: {}", e);
    }
}

/// Convert a SDK [`LogEntry`] to the protobuf [`LogEntry`](proto::LogEntry) representation.
pub(super) fn log_entry_to_proto(entry: LogEntry) -> proto::LogEntry {
    proto::LogEntry {
        timestamp_ns: entry.timestamp_ns,
        level: entry.level as i32,
        target: entry.target,
        message: entry.message,
        fields: entry.fields,
    }
}
