//! The core HostPlugin trait that all malbox host plugins implement.

use crate::context::Context;
use crate::error::Result;
use crate::types::{HealthStatus, Task};
use malbox_plugin_transport::messages::events::Event;
use std::collections::HashMap;

/// The trait that all malbox host plugins implement.
///
/// All methods have default implementations, but a useful plugin should
/// at minimum implement `on_task`. The `#[malbox::handlers]` macro generates
/// this impl from annotated methods on your plugin struct.
pub trait HostPlugin: Send + Sync + 'static {
    /// Process an analysis task.
    ///
    /// Emit results via [`Context::push_result`](crate::context::Context::push_result).
    /// The runtime sends a final marker automatically once this method returns.
    fn on_task(&self, task: Task, ctx: &Context) -> Result<()> {
        let _ = (task, ctx);
        Ok(())
    }

    /// Called once when the plugin is initialized.
    fn on_start(&self, config: HashMap<String, String>) -> Result<()> {
        let _ = config;
        Ok(())
    }

    /// Called once when the plugin is shutting down.
    fn on_stop(&self) -> Result<()> {
        Ok(())
    }

    /// Return the plugin's current health status.
    fn health_check(&self) -> HealthStatus {
        HealthStatus::ready()
    }

    /// Handle a system event.
    fn on_event(&self, event: Event, ctx: &Context) -> Result<()> {
        let _ = (event, ctx);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PluginResult;

    // MinimalPlugin: empty impl -- verify all defaults work
    struct MinimalPlugin;
    impl HostPlugin for MinimalPlugin {}

    #[test]
    fn minimal_plugin_defaults_work() {
        let plugin = MinimalPlugin;

        // on_start default succeeds
        assert!(plugin.on_start(HashMap::new()).is_ok());

        // on_stop default succeeds
        assert!(plugin.on_stop().is_ok());

        // health_check default returns ready
        let health = plugin.health_check();
        assert!(health.ready);
    }

    // TaskOnlyPlugin: verify on_task override works
    struct TaskOnlyPlugin;
    impl HostPlugin for TaskOnlyPlugin {
        fn on_task(&self, _task: Task, ctx: &Context) -> Result<()> {
            ctx.push_result(PluginResult::bytes("hello", vec![1, 2, 3]))?;
            Ok(())
        }
    }

    #[test]
    fn task_only_plugin_override_works() {
        let plugin = TaskOnlyPlugin;
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let emitter = ();
        let ctx = Context::new(&emitter, Some(tx)).with_task_id(1);
        let task = Task::new(1, std::path::PathBuf::from("/tmp/sample"), HashMap::new());

        plugin.on_task(task, &ctx).expect("on_task should succeed");

        let received = rx.try_recv().expect("one result expected");
        let task_result = received.expect("should be Ok");
        assert_eq!(task_result.result_name, "hello");
        assert_eq!(task_result.data, vec![1, 2, 3]);
    }

    #[test]
    fn on_event_default_is_noop() {
        let plugin = MinimalPlugin;
        let emitter = ();
        let ctx = Context::new(&emitter, None);
        let event = Event::TaskCreated { task_id: 42 };

        let result = plugin.on_event(event, &ctx);
        assert!(result.is_ok());
    }
}
