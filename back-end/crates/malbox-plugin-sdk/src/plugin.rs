//! The core HostPlugin trait that all malbox host plugins implement.

use crate::context::Context;
use crate::error::Result;
use crate::types::{ExecRequest, ExecResult, HealthStatus, Task};
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

    /// Optionally override command execution on the guest OS.
    /// Return `Some(result)` to handle it, `None` for runtime default.
    fn on_execute_command(&self, request: &ExecRequest) -> Option<ExecResult> {
        let _ = request;
        None
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

        // on_execute_command default returns None
        let req = ExecRequest::new("cmd".into(), vec![], None, HashMap::new(), None, false);
        assert!(plugin.on_execute_command(&req).is_none());
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
        let ctx = Context::new(&emitter, Some(tx), None).with_task_id(1);
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
        let ctx = Context::new(&emitter, None, None);
        let event = Event::TaskCreated { task_id: 42 };

        let result = plugin.on_event(event, &ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn on_execute_command_default_returns_none() {
        let plugin = MinimalPlugin;
        let req = ExecRequest::new(
            "notepad.exe".into(),
            vec!["file.txt".into()],
            Some("C:\\".into()),
            HashMap::new(),
            Some(std::time::Duration::from_secs(30)),
            true,
        );
        assert!(plugin.on_execute_command(&req).is_none());
    }
}
