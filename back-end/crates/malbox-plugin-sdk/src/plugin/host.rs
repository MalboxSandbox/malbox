//! The [`HostPlugin`] trait for plugins that run on the daemon host.
//!
//! Host plugins process tasks via [`on_task`](HostPlugin::on_task) and
//! can subscribe to system events via
//! [`on_event`](HostPlugin::on_event). The `#[malbox::handlers]` macro
//! generates this impl from annotated methods on your plugin struct.

use crate::context::Context;
use crate::error::Result;
use malbox_plugin_transport::messages::events::Event;
use std::collections::HashMap;

use super::Plugin;

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

    /// Called once when the plugin process starts. `config` contains any
    /// key-value settings the daemon passes to this plugin.
    fn on_start(&self, config: HashMap<String, String>) -> Result<()> {
        let _ = config;
        Ok(())
    }

    /// Called once when the daemon is shutting down this plugin. Use it
    /// to flush buffers, close connections, or clean up resources.
    fn on_stop(&self) -> Result<()> {
        Ok(())
    }

    /// Handle a system or plugin event.
    ///
    /// All events (task lifecycle, plugin lifecycle, system events, and
    /// plugin result availability) are delivered through this single handler.
    /// Use `#[malbox::on_event(...)]` with filters in the macro to route
    /// specific events to specific methods.
    fn on_event(&self, event: Event) -> Result<()> {
        let _ = event;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::result::PluginResult;
    use std::collections::HashMap;
    use std::sync::Arc;

    struct MinimalPlugin;
    impl Plugin for MinimalPlugin {}
    impl HostPlugin for MinimalPlugin {}

    #[test]
    fn minimal_plugin_defaults_work() {
        let plugin = MinimalPlugin;

        assert!(plugin.on_start(HashMap::new()).is_ok());
        assert!(plugin.on_stop().is_ok());

        let health = plugin.health_check();
        assert!(health.is_ready());
    }

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
            #[cfg(feature = "guest")]
            None,
        );

        plugin.on_task(&ctx).expect("on_task should succeed");

        let msg = rx.try_recv().expect("one result expected");
        assert_eq!(msg.result_name, "hello");
        assert_eq!(msg.data, vec![1, 2, 3]);
    }

    #[test]
    fn on_event_default_is_noop() {
        let plugin = MinimalPlugin;
        let event = Event::TaskCreated { task_id: 42 };

        let result = plugin.on_event(event);
        assert!(result.is_ok());
    }
}
