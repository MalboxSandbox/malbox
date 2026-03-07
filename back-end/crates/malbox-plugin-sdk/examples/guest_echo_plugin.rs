//! Example guest plugin that echoes task events over gRPC.
//!
//! This is the gRPC equivalent of `echo_plugin.rs`. The same `Plugin` trait
//! implementation works with both IPC and gRPC runtimes.
//!
//! ```sh
//! cargo run -p malbox-plugin-sdk --features guest --example guest_echo_plugin
//! ```

use malbox_plugin_sdk::prelude::*;
use tracing::info;

struct EchoPlugin;

impl Plugin for EchoPlugin {
    fn name(&self) -> &str {
        "guest-echo-plugin"
    }

    fn plugin_id(&self) -> i32 {
        1
    }

    fn on_start(&self, _ctx: &EventContext) -> Result<()> {
        info!("Guest echo plugin starting");
        Ok(())
    }

    fn on_stop(&self, _ctx: &EventContext) -> Result<()> {
        info!("Guest echo plugin stopping");
        Ok(())
    }

    fn on_task_event(
        &self,
        event: TaskEvent,
        payload: TaskEventPayload,
        ctx: &EventContext,
    ) -> Result<()> {
        info!("Task event {:?} for task_id={}", event, payload.task_id);

        ctx.emit_plugin_started(self.plugin_id())?;
        info!("Emitted PluginStarted");

        // Simulate work
        std::thread::sleep(std::time::Duration::from_millis(200));

        ctx.emit_result_produced(self.plugin_id())?;
        info!("Emitted PluginResultProduced");

        ctx.emit_plugin_stopped(self.plugin_id())?;
        info!("Emitted PluginStopped");

        Ok(())
    }
}

fn main() {
    malbox_tracing::init_tracing("debug");

    info!("Starting guest echo plugin on 0.0.0.0:50051...");

    let config = GuestRuntimeConfig {
        work_dir: std::path::PathBuf::from("/tmp/malbox-echo"),
        ..Default::default()
    };

    let runtime = GuestPluginRuntime::with_config(EchoPlugin, config);
    runtime.run_blocking().expect("Guest plugin runtime error");
}
