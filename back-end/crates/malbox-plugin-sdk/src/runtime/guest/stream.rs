//! Result streaming + log entry conversion for the guest runtime.
//!
//! The guest linear task runs a [`GuestPlugin`]'s lifecycle sequence
//! (`on_start` -> `execute_sample` -> wait -> `on_stop`) and streams
//! results back over an mpsc channel. Auto-collection of artifacts and
//! external logs is handled after `on_stop`.

use crate::context::message::{ResultFormat, ResultKind, TaskResultMessage};
use crate::context::{Context, ResultSender};
use crate::log::LogEntry;
use crate::plugin::guest::{GuestPlugin, LaunchResult};
use crate::stash::ResultStash;
use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::plugin::GrpcEmitter;
use malbox_plugin_transport::traits::TransportEmitter;
use prost::Message;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::Status;
use tracing::{debug, error, info, instrument};

use super::collector::{self, AutoCollectSection};

/// All parameters for a single guest task execution (everything except the
/// generic plugin instance).
pub(super) struct TaskExecution {
    pub task_id: i32,
    pub sample_path: PathBuf,
    pub config: HashMap<String, String>,
    pub result_tx: ResultSender,
    pub proto_tx: mpsc::Sender<std::result::Result<proto::TaskResult, Status>>,
    pub stash: Arc<ResultStash>,
    pub artifact_dir: PathBuf,
    pub external_log_dir: PathBuf,
    pub auto_collect_artifacts: AutoCollectSection,
    pub auto_collect_external_logs: AutoCollectSection,
    pub default_timeout: u64,
}

/// Convert a [`TaskResultMessage`] to the protobuf `TaskResult` representation.
pub(super) fn message_to_proto(msg: TaskResultMessage, _stash: &ResultStash) -> proto::TaskResult {
    match msg.kind {
        ResultKind::ResultRef => {
            let ref_msg = proto::ResultRef {
                handle: msg.stash_handle,
                result_name: msg.result_name,
                format: result_format_to_proto(msg.stash_format).into(),
                size_bytes: msg.stash_size,
            };
            proto::TaskResult {
                task_id: msg.task_id,
                result_name: String::new(),
                data: ref_msg.encode_to_vec(),
                format: proto::ResultFormat::Unspecified.into(),
                is_final: msg.is_final,
                kind: proto::ResultKind::ResultRef.into(),
            }
        }
        ResultKind::Progress => proto::TaskResult {
            task_id: msg.task_id,
            result_name: msg.result_name,
            data: msg.data,
            format: result_format_to_proto(msg.format).into(),
            is_final: msg.is_final,
            kind: proto::ResultKind::Progress.into(),
        },
        ResultKind::Result => proto::TaskResult {
            task_id: msg.task_id,
            result_name: msg.result_name,
            data: msg.data,
            format: result_format_to_proto(msg.format).into(),
            is_final: msg.is_final,
            kind: proto::ResultKind::Result.into(),
        },
    }
}

fn result_format_to_proto(format: ResultFormat) -> proto::ResultFormat {
    match format {
        ResultFormat::Json => proto::ResultFormat::Json,
        ResultFormat::Bytes => proto::ResultFormat::Bytes,
        ResultFormat::Unspecified => proto::ResultFormat::Unspecified,
    }
}

/// Send the final-marker `TaskResult` that signals end-of-stream to the daemon.
fn send_final_marker(
    task_id: i32,
    proto_tx: &mpsc::Sender<std::result::Result<proto::TaskResult, Status>>,
) {
    let final_marker = proto::TaskResult {
        task_id,
        result_name: String::new(),
        data: vec![],
        format: proto::ResultFormat::Unspecified.into(),
        is_final: true,
        kind: proto::ResultKind::Result.into(),
    };
    let _ = proto_tx.blocking_send(Ok(final_marker));
}

/// Run a [`GuestPlugin`]'s linear lifecycle and stream results back over
/// the task's result channel.
///
/// The sequence is:
/// 1. Send READY signal
/// 2. `on_start` - plugin sets up monitoring
/// 3. `execute_sample` - SDK launches the sample
/// 4. Wait for analysis timeout
/// 5. `on_stop` - plugin flushes results
/// 6. Auto-collect artifacts and external logs
/// 7. Send final marker
#[instrument(skip_all, fields(task_id = exec.task_id))]
pub(super) fn guest_linear_task<P: GuestPlugin>(plugin: Arc<P>, exec: TaskExecution) {
    let TaskExecution {
        task_id,
        sample_path,
        config,
        result_tx,
        proto_tx,
        stash,
        artifact_dir,
        external_log_dir,
        auto_collect_artifacts,
        auto_collect_external_logs,
        default_timeout,
    } = exec;

    let ready_result = proto::TaskResult {
        task_id,
        result_name: String::new(),
        data: vec![],
        format: proto::ResultFormat::Unspecified.into(),
        is_final: false,
        kind: proto::ResultKind::Ready.into(),
    };
    if let Err(e) = proto_tx.blocking_send(Ok(ready_result)) {
        error!(error = %e, "Failed to send READY signal");
        return;
    }

    let emitter: Arc<dyn TransportEmitter + Send + Sync> =
        Arc::new(GrpcEmitter::with_task(task_id, proto_tx.clone()));

    let ctx = Context::new(
        task_id,
        sample_path.clone(),
        config.clone(),
        emitter,
        Some(result_tx.clone()),
        Some(Arc::clone(&stash)),
    );

    if let Err(e) = plugin.on_start(&ctx) {
        error!(error = %e, "GuestPlugin on_start failed");
        send_final_marker(task_id, &proto_tx);
        return;
    }

    info!(path = %sample_path.display(), "Calling execute_sample");
    let launched = match plugin.execute_sample(&sample_path) {
        Ok(LaunchResult::Launched) => {
            info!(path = %sample_path.display(), "Sample launched successfully");
            true
        }
        Ok(LaunchResult::UseDefault) => {
            info!(path = %sample_path.display(), "Plugin returned UseDefault, using default launcher");
            match crate::plugin::guest::default_launch(&sample_path) {
                LaunchResult::Launched => {
                    info!(path = %sample_path.display(), "Default launch succeeded");
                    true
                }
                LaunchResult::UseDefault => {
                    error!(path = %sample_path.display(), "Default launch also returned UseDefault, sample was never executed");
                    false
                }
            }
        }
        Err(e) => {
            error!(path = %sample_path.display(), error = %e, "execute_sample failed, sample was never executed");
            false
        }
    };

    if launched {
        let timeout = config
            .get("analysis_timeout")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(default_timeout);
        info!(timeout_secs = timeout, "Waiting for analysis timeout");
        std::thread::sleep(std::time::Duration::from_secs(timeout));
    }

    if let Err(e) = plugin.on_stop(&ctx) {
        error!(error = %e, "GuestPlugin on_stop failed");
    }

    if launched && auto_collect_artifacts.enabled {
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

    if launched && auto_collect_external_logs.enabled {
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

    // Drain the internal result channel and convert to proto
    drop(result_tx);

    send_final_marker(task_id, &proto_tx);
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
