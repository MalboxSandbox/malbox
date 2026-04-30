//! The core Plugin and HostPlugin traits that all malbox plugins implement.

use crate::context::Context;
use crate::error::Result;
use crate::types::HealthStatus;
use malbox_plugin_transport::messages::events::Event;
use std::collections::HashMap;

/// Base trait for all Malbox plugins.
pub trait Plugin: Send + Sync + 'static {
    /// Return the plugin's current health status.
    fn health_check(&self) -> HealthStatus {
        HealthStatus::ready()
    }
}

/// The trait that all malbox host plugins implement.
///
/// All methods have default implementations, but a useful plugin should
/// at minimum implement `on_task`. The `#[malbox::handlers]` macro generates
/// this impl from annotated methods on your plugin struct.
pub trait HostPlugin: Plugin {
    /// Process an analysis task.
    ///
    /// Emit results via [`Context::results().push()`](crate::context::ResultSink::push).
    /// The runtime sends a final marker automatically once this method returns.
    fn on_task(&self, ctx: &Context) -> Result<()> {
        let _ = ctx;
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

    /// Handle a system event.
    fn on_event(&self, event: Event) -> Result<()> {
        let _ = event;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PluginResult;
    use std::collections::HashMap;
    use std::sync::Arc;

    // MinimalPlugin: empty impl -- verify all defaults work
    struct MinimalPlugin;
    impl Plugin for MinimalPlugin {}
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
    impl Plugin for TaskOnlyPlugin {}
    impl HostPlugin for TaskOnlyPlugin {
        fn on_task(&self, ctx: &Context) -> Result<()> {
            ctx.results()
                .push(PluginResult::bytes("hello", vec![1, 2, 3]))?;
            Ok(())
        }
    }

    fn noop_emitter() -> Arc<dyn malbox_plugin_transport::traits::TransportEmitter + Send + Sync> {
        Arc::new(())
    }

    #[test]
    fn task_only_plugin_override_works() {
        let plugin = TaskOnlyPlugin;
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let ctx = Context::new(
            1,
            std::path::PathBuf::new(),
            HashMap::new(),
            noop_emitter(),
            Some(tx),
            None,
        );

        plugin.on_task(&ctx).expect("on_task should succeed");

        let received = rx.try_recv().expect("one result expected");
        let task_result = received.expect("should be Ok");
        assert_eq!(task_result.result_name, "hello");
        assert_eq!(task_result.data, vec![1, 2, 3]);
    }

    #[test]
    fn on_event_default_is_noop() {
        let plugin = MinimalPlugin;
        let event = Event::TaskCreated { task_id: 42 };

        let result = plugin.on_event(event);
        assert!(result.is_ok());
    }
}
