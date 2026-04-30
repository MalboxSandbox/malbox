//! Result streaming + log entry conversion for the guest runtime.
//!
//! The guest linear task runs a [`GuestPlugin`]'s lifecycle sequence
//! (`on_start` -> `execute_sample` -> wait -> `on_stop`) and streams
//! results back over an mpsc channel. Auto-collection of artifacts and
//! external logs is handled after `on_stop`.

use crate::context::Context;
use crate::guest_plugin::GuestPlugin;
use crate::log::LogEntry;
use crate::stash::ResultStash;
use crate::types::Task;
use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::plugin::GrpcEmitter;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::Status;
use tracing::{debug, error, info, instrument};

use super::collector::{self, AutoCollectSection};

/// Send the final-marker `TaskResult` that signals end-of-stream to the daemon.
fn send_final_marker(
    task_id: i32,
    result_tx: &mpsc::Sender<std::result::Result<proto::TaskResult, Status>>,
) {
    let final_marker = proto::TaskResult {
        task_id,
        result_name: String::new(),
        data: vec![],
        format: proto::ResultFormat::Unspecified.into(),
        is_final: true,
        kind: proto::ResultKind::Result.into(),
    };
    let _ = result_tx.blocking_send(Ok(final_marker));
}

/// Run a [`GuestPlugin`]'s linear lifecycle and stream results back over
/// `result_tx`.
///
/// The sequence is:
/// 1. Send READY signal
/// 2. `on_start` - plugin sets up monitoring
/// 3. `execute_sample` - SDK launches the sample
/// 4. Wait for analysis timeout
/// 5. `on_stop` - plugin flushes results
/// 6. Auto-collect artifacts and external logs
/// 7. Send final marker
#[instrument(skip_all, fields(task_id))]
#[allow(clippy::too_many_arguments)]
pub(super) fn guest_linear_task<P: GuestPlugin>(
    plugin: Arc<P>,
    task_id: i32,
    sample_path: PathBuf,
    config: HashMap<String, String>,
    result_tx: mpsc::Sender<std::result::Result<proto::TaskResult, Status>>,
    stash: Arc<ResultStash>,
    artifact_dir: PathBuf,
    external_log_dir: PathBuf,
    auto_collect_artifacts: AutoCollectSection,
    auto_collect_external_logs: AutoCollectSection,
    default_timeout: u64,
) {
    // (0) Send READY signal to indicate the task handler is about to start.
    let ready_result = proto::TaskResult {
        task_id,
        result_name: String::new(),
        data: vec![],
        format: proto::ResultFormat::Unspecified.into(),
        is_final: false,
        kind: proto::ResultKind::Ready.into(),
    };
    if let Err(e) = result_tx.blocking_send(Ok(ready_result)) {
        error!(error = %e, "Failed to send READY signal");
        return;
    }

    let emitter = GrpcEmitter::with_task(task_id, result_tx.clone());
    let ctx = Context::new(&emitter, Some(result_tx.clone()))
        .with_task_id(task_id)
        .with_stash(Arc::clone(&stash));

    let task = Task::new(task_id, sample_path.clone(), config.clone());

    // (1) on_start - plugin sets up monitoring
    if let Err(e) = plugin.on_start(&task, &ctx) {
        error!(error = %e, "GuestPlugin on_start failed");
        send_final_marker(task_id, &result_tx);
        return;
    }

    // (2) execute_sample - SDK launches the sample
    info!(path = %sample_path.display(), "Calling execute_sample");
    let launched = plugin.execute_sample(&sample_path);
    if launched {
        info!(path = %sample_path.display(), "Sample launched successfully");
    } else {
        error!(path = %sample_path.display(), "execute_sample failed - sample was NOT launched");
    }

    // (3) Wait for analysis timeout
    let timeout = config
        .get("analysis_timeout")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default_timeout);
    info!(timeout_secs = timeout, "Waiting for analysis timeout");
    std::thread::sleep(std::time::Duration::from_secs(timeout));

    // (4) on_stop - plugin flushes results
    if let Err(e) = plugin.on_stop(&ctx) {
        error!(error = %e, "GuestPlugin on_stop failed");
    }

    // (5) Auto-collect artifacts
    if auto_collect_artifacts.enabled {
        debug!("auto-collecting artifacts from {}", artifact_dir.display());
        let claimed = ctx.claimed_paths();
        collector::auto_collect(
            &ctx,
            &artifact_dir,
            &auto_collect_artifacts,
            Some(&claimed),
            "artifacts",
        );
    }

    // (6) Auto-collect external logs
    if auto_collect_external_logs.enabled {
        debug!(
            "auto-collecting external logs from {}",
            external_log_dir.display()
        );
        collector::auto_collect(
            &ctx,
            &external_log_dir,
            &auto_collect_external_logs,
            None,
            "ext-logs",
        );
    }

    // (7) Send final marker
    send_final_marker(task_id, &result_tx);
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
