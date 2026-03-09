extern crate malbox_plugin_sdk as malbox;

use malbox::prelude::*;
use std::sync::Mutex;

/// A report-aggregator–style host plugin that demonstrates the `#[on_event]`
/// system, inspired by the design doc's "event reactor" pattern.
///
/// Instead of processing tasks directly, this plugin reacts to lifecycle
/// events emitted by the daemon and other plugins — collecting results as
/// they arrive and building a combined report when a task finishes.
#[malbox::host_plugin]
#[malbox(state = "persistent", execution = "unrestricted")]
struct EventReactor {
    /// Accumulated result count per task (simple in-memory tracking).
    task_results: Mutex<Vec<TaskReport>>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TaskReport {
    task_id: i32,
    results_collected: u32,
    status: &'static str,
}

#[malbox::handlers]
impl EventReactor {
    #[malbox::on_start]
    fn init(&self) -> Result<()> {
        info!("EventReactor started — listening for system events");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Daemon events
    // -----------------------------------------------------------------------

    /// React to configuration changes — no parameters needed, the macro
    /// wraps the bare `()` return with `Ok(())` automatically.
    #[malbox::on_event(DaemonEvent::ConfigReloaded)]
    fn on_config_reload(&self) {
        info!("Configuration reloaded — re-reading settings");
    }

    // -----------------------------------------------------------------------
    // Plugin events — the `from` filter declares interest in specific plugins.
    // The filter is parsed and will be used for subscription routing once
    // the transport layer supports source metadata.
    // -----------------------------------------------------------------------

    /// Called when any plugin produces a result. The `from` filter narrows
    /// this to results from `host-file-info` and `guest-yara-scanner` only.
    #[malbox::on_event(PluginEvent::PluginResultProduced, from = ["host-file-info", "guest-yara-scanner"])]
    fn on_result(&self, payload: PluginEventPayload, _ctx: &Context) -> Result<()> {
        info!(
            plugin_id = payload.plugin_id,
            "Got result from watched plugin — tracking for report aggregation"
        );
        // In a real plugin you'd fetch the actual result via ctx.get_result()
        // and merge it into your report. For now, bump a counter.
        if let Ok(mut reports) = self.task_results.lock() {
            // Simplified: just track that we saw a result
            reports.push(TaskReport {
                task_id: 0, // would come from richer payload in the future
                results_collected: 1,
                status: "collecting",
            });
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Task lifecycle events — multiple handlers for the same category
    // generate a single trait impl with a combined match.
    // -----------------------------------------------------------------------

    /// When a task completes, build a combined report from all the results
    /// we've collected and emit it.
    #[malbox::on_event(TaskEvent::TaskCompleted)]
    fn on_task_done(&self, payload: TaskEventPayload, _ctx: &Context) -> Result<()> {
        let count = self
            .task_results
            .lock()
            .map(|r| r.len())
            .unwrap_or(0);

        info!(
            task_id = payload.task_id,
            results_collected = count,
            "Task completed — building aggregated report"
        );

        // In a real plugin, you'd emit a PluginResult here:
        //   let report = self.build_report(payload.task_id)?;
        //   Ok(vec![PluginResult::json("full_report", &report)?])

        Ok(())
    }

    /// Log task failures for monitoring/alerting.
    #[malbox::on_event(TaskEvent::TaskFailed)]
    fn on_task_failed(&self, payload: TaskEventPayload, _ctx: &Context) -> Result<()> {
        warn!(
            task_id = payload.task_id,
            "Task failed — could trigger retry or alerting logic"
        );
        Ok(())
    }

    #[malbox::on_stop]
    fn shutdown(&self) -> Result<()> {
        let count = self
            .task_results
            .lock()
            .map(|r| r.len())
            .unwrap_or(0);
        info!(total_results = count, "EventReactor shutting down");
        Ok(())
    }
}
