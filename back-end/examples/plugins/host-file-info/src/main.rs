//! Example host plugin — file information extractor.
//!
//! Runs on the host alongside the daemon, communicating via IPC.
//! When a task is created it computes the SHA-256 hash and file size
//! of the sample, then emits a result.
//!
//! ```sh
//! cargo run --manifest-path examples/plugins/host-file-info/Cargo.toml
//! ```

use malbox_plugin_sdk::prelude::*;
use sha2::{Digest, Sha256};
use tracing::info;

struct FileInfoPlugin;

impl Plugin for FileInfoPlugin {
    fn name(&self) -> &str {
        "host-file-info"
    }

    fn plugin_id(&self) -> i32 {
        100
    }

    fn on_start(&self, _ctx: &EventContext) -> Result<()> {
        info!("FileInfo plugin ready — waiting for tasks");
        Ok(())
    }

    fn on_task_event(
        &self,
        event: TaskEvent,
        payload: TaskEventPayload,
        ctx: &EventContext,
    ) -> Result<()> {
        if !matches!(event, TaskEvent::TaskCreated) {
            return Ok(());
        }

        let task_id = payload.task_id;
        info!(task_id, "Processing new task");

        ctx.emit_plugin_started(self.plugin_id())?;

        // In a real plugin you would read the sample path from the task
        // configuration and hash the actual file. Here we simulate it.
        let sample_bytes: &[u8] = b"<sample-bytes-placeholder>";
        let hash = hex_sha256(sample_bytes);
        let size = sample_bytes.len();

        info!(task_id, %hash, size, "Computed file info");

        ctx.emit_result_produced(self.plugin_id())?;
        ctx.emit_plugin_stopped(self.plugin_id())?;

        info!(task_id, "Task processing complete");
        Ok(())
    }

    fn on_daemon_event(&self, event: DaemonEvent, _ctx: &EventContext) -> Result<()> {
        match event {
            DaemonEvent::ConfigReloaded => info!("Configuration reloaded"),
            DaemonEvent::DaemonShutdown => info!("Daemon shutting down"),
        }
        Ok(())
    }
}

fn hex_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn main() {
    malbox_tracing::init_tracing("debug");
    info!("Starting host-file-info plugin");

    let runtime = PluginRuntime::new(FileInfoPlugin).expect("Failed to create plugin runtime");
    runtime.run().expect("Plugin runtime error");
}
