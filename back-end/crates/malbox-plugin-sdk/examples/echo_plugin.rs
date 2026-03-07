//! Example plugin that echoes TaskCreated events.
//!
//! Run the daemon first, then this binary. Submit a task via HTTP
//! and watch the events flow bidirectionally.
//!
//! ```sh
//! cargo run -p malbox-plugin-sdk --example echo_plugin
//! ```

use malbox_plugin_sdk::prelude::*;
use tracing::info;

struct EchoPlugin;

impl Plugin for EchoPlugin {
    fn name(&self) -> &str {
        "echo-plugin"
    }

    fn plugin_id(&self) -> i32 {
        1
    }

    fn on_task_event(
        &self,
        event: TaskEvent,
        payload: TaskEventPayload,
        ctx: &EventContext,
    ) -> Result<()> {
        match event {
            TaskEvent::TaskCreated => {
                info!("TaskCreated for task_id={}", payload.task_id);

                ctx.emit_plugin_started(self.plugin_id())?;
                info!("Emitted PluginStarted");

                // Simulate work
                std::thread::sleep(std::time::Duration::from_millis(500));

                ctx.emit_result_produced(self.plugin_id())?;
                info!("Emitted PluginResultProduced");

                ctx.emit_plugin_stopped(self.plugin_id())?;
                info!("Emitted PluginStopped");

                info!("Finished processing task_id={}", payload.task_id);
            }
            other => {
                info!("Task event {:?} for task_id={}", other, payload.task_id);
            }
        }
        Ok(())
    }
}

fn main() {
    malbox_tracing::init_tracing("debug");

    info!("Starting echo plugin...");

    let runtime = PluginRuntime::new(EchoPlugin).expect("Failed to create plugin runtime");
    runtime.run().expect("Plugin runtime error");
}
