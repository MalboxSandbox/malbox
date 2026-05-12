extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};

/// A host plugin that demonstrates the event system by reacting to lifecycle
/// events emitted by the daemon and other plugins.
///
/// Instead of processing tasks directly, this plugin tracks system activity
/// and logs when interesting things happen.
#[malbox::host_plugin]
struct EventReactor {
    results_seen: AtomicU32,
}

#[malbox::handlers]
impl EventReactor {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        info!("EventReactor started - listening for system events");
        Ok(())
    }

    #[malbox::on_task]
    fn handle_task(&self, _ctx: &Context) -> Result<()> {
        Ok(())
    }

    #[malbox::on_event(ConfigReloaded)]
    fn on_config_reload(&self) {
        info!("Configuration reloaded");
    }

    #[malbox::on_event(PluginResultAvailable)]
    fn on_result_available(&self) -> Result<()> {
        let count = self.results_seen.fetch_add(1, Ordering::Relaxed) + 1;
        info!(total = count, "Plugin result available - tracking");
        Ok(())
    }

    #[malbox::on_event(TaskCompleted)]
    fn on_task_done(&self) -> Result<()> {
        let count = self.results_seen.load(Ordering::Relaxed);
        info!(results_collected = count, "Task completed");
        Ok(())
    }

    #[malbox::on_event(TaskFailed)]
    fn on_task_failed(&self) {
        warn!("Task failed - could trigger alerting logic");
    }

    #[malbox::on_stop]
    fn shutdown(&self) -> Result<()> {
        let count = self.results_seen.load(Ordering::Relaxed);
        info!(total_results = count, "EventReactor shutting down");
        Ok(())
    }
}
