//! Daemon-side log router for guest plugin log streams.
//!
//! When the daemon opens a `StreamLogs` RPC to a guest plugin, the returned
//! `tonic::Streaming<LogEntry>` is handed to [`spawn_log_consumer`] which
//! spawns a background task that:
//!
//! 1. Forwards each log entry to the daemon's own tracing system so operators
//!    see plugin output in the daemon log.
//! 2. Writes each entry to a per-plugin, per-run log file for post-mortem
//!    analysis.

use std::io::Write as _;
use std::path::PathBuf;

use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};

use crate::registry::types::PluginId;
use crate::transport::grpc::proto;

/// Spawn a background task that consumes a guest plugin's log stream.
///
/// The task reads from the provided `tonic::Streaming<proto::LogEntry>`,
/// forwards each entry to the daemon's tracing infrastructure and writes it
/// to a log file at `<log_dir>/<plugin_id>/<run_id>.log`.
///
/// Returns the join handle for the consumer task and the path to the log file.
pub fn spawn_log_consumer(
    plugin_id: PluginId,
    mut stream: tonic::Streaming<proto::LogEntry>,
    log_dir: PathBuf,
    run_id: String,
) -> (JoinHandle<()>, PathBuf) {
    let dir = log_dir.join(plugin_id.as_str());
    let log_file_path = dir.join(format!("{run_id}.log"));
    let returned_path = log_file_path.clone();

    let handle = tokio::spawn(async move {
        // Create the directory tree and open the log file.
        if let Err(e) = std::fs::create_dir_all(&dir) {
            warn!(
                plugin = %plugin_id,
                error = %e,
                path = %dir.display(),
                "failed to create log directory, file logging disabled"
            );
        }

        let file = match std::fs::File::create(&log_file_path) {
            Ok(f) => f,
            Err(e) => {
                warn!(
                    plugin = %plugin_id,
                    error = %e,
                    path = %log_file_path.display(),
                    "failed to create log file, file logging disabled"
                );
                // Continue without file logging — tracing forwarding still works.
                consume_stream_no_file(&plugin_id, &mut stream).await;
                return;
            }
        };

        let mut writer = std::io::BufWriter::new(file);

        info!(
            target: "guest",
            plugin = %plugin_id,
            path = %log_file_path.display(),
            "Guest plugin log file opened"
        );

        loop {
            match stream.message().await {
                Ok(Some(entry)) => {
                    forward_to_tracing(&plugin_id, &entry);
                    write_to_file(&mut writer, &entry);
                    let _ = writer.flush();
                }
                Ok(None) => {
                    debug!(plugin = %plugin_id, "log stream ended (EOF)");
                    break;
                }
                Err(e) => {
                    warn!(plugin = %plugin_id, error = %e, "log stream error, stopping consumer");
                    break;
                }
            }
        }

        // Flush any remaining buffered data.
        let _ = writer.flush();
    });

    (handle, returned_path)
}

/// Consume the stream forwarding only to tracing (when file creation failed).
async fn consume_stream_no_file(
    plugin_id: &PluginId,
    stream: &mut tonic::Streaming<proto::LogEntry>,
) {
    loop {
        match stream.message().await {
            Ok(Some(entry)) => {
                forward_to_tracing(plugin_id, &entry);
            }
            Ok(None) => {
                debug!(plugin = %plugin_id, "log stream ended (EOF)");
                break;
            }
            Err(e) => {
                warn!(plugin = %plugin_id, error = %e, "log stream error, stopping consumer");
                break;
            }
        }
    }
}

/// Forward a single log entry to the daemon's tracing system.
///
/// Maps the proto `LogLevel` back to the corresponding tracing macro and emits
/// the message with `plugin` and `target` span fields so operators can filter
/// guest plugin output in the daemon log.
fn forward_to_tracing(plugin_id: &PluginId, entry: &proto::LogEntry) {
    let level = proto::LogLevel::try_from(entry.level).unwrap_or(proto::LogLevel::Info);
    let target = &entry.target;

    let message = if entry.fields.is_empty() {
        entry.message.clone()
    } else {
        let fields: Vec<String> = entry
            .fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!("{} {{{}}}", entry.message, fields.join(", "))
    };

    match level {
        proto::LogLevel::Trace => {
            trace!(
                target: "guest",
                plugin = %plugin_id,
                guest_target = %target,
                "{}", message
            );
        }
        proto::LogLevel::Debug => {
            debug!(
                target: "guest",
                plugin = %plugin_id,
                guest_target = %target,
                "{}", message
            );
        }
        proto::LogLevel::Info => {
            info!(
                target: "guest",
                plugin = %plugin_id,
                guest_target = %target,
                "{}", message
            );
        }
        proto::LogLevel::Warn => {
            warn!(
                target: "guest",
                plugin = %plugin_id,
                guest_target = %target,
                "{}", message
            );
        }
        proto::LogLevel::Error => {
            error!(
                target: "guest",
                plugin = %plugin_id,
                guest_target = %target,
                "{}", message
            );
        }
    }
}

/// Write a single log entry to the log file.
///
/// Format: `[timestamp_ns] [LEVEL] [target] message {fields}`
fn write_to_file(writer: &mut std::io::BufWriter<std::fs::File>, entry: &proto::LogEntry) {
    let level = proto::LogLevel::try_from(entry.level).unwrap_or(proto::LogLevel::Info);
    let level_str = match level {
        proto::LogLevel::Trace => "TRACE",
        proto::LogLevel::Debug => "DEBUG",
        proto::LogLevel::Info => "INFO",
        proto::LogLevel::Warn => "WARN",
        proto::LogLevel::Error => "ERROR",
    };

    let fields_str = if entry.fields.is_empty() {
        String::new()
    } else {
        let pairs: Vec<String> = entry
            .fields
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!(" {{{}}}", pairs.join(", "))
    };

    let _ = writeln!(
        writer,
        "[{}] [{}] [{}] {}{}",
        entry.timestamp_ns, level_str, entry.target, entry.message, fields_str
    );
}
