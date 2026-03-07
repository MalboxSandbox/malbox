//! Plugin trait and event context.

use crate::error::{Result, SdkError};
use malbox_plugin_transport::messages::events::*;
use malbox_plugin_transport::traits::TransportEmitter;

/// Context provided to plugin event handlers for sending events back to the daemon.
pub struct EventContext<'a> {
    emitter: &'a dyn TransportEmitter,
}

impl<'a> EventContext<'a> {
    pub(crate) fn new(emitter: &'a dyn TransportEmitter) -> Self {
        Self { emitter }
    }

    /// Emit a raw event and payload back to the daemon.
    pub fn emit(&self, event: Event, payload: Payload) -> Result<()> {
        self.emitter
            .emit(event, payload)
            .map_err(SdkError::Transport)
    }

    /// Emit PluginStarted event.
    pub fn emit_plugin_started(&self, plugin_id: i32) -> Result<()> {
        self.emit(
            Event::Plugin(PluginEvent::PluginStarted),
            Payload::Plugin(PluginEventPayload { plugin_id }),
        )
    }

    /// Emit PluginStopped event.
    pub fn emit_plugin_stopped(&self, plugin_id: i32) -> Result<()> {
        self.emit(
            Event::Plugin(PluginEvent::PluginStopped),
            Payload::Plugin(PluginEventPayload { plugin_id }),
        )
    }

    /// Emit PluginResultProduced event.
    pub fn emit_result_produced(&self, plugin_id: i32) -> Result<()> {
        self.emit(
            Event::Plugin(PluginEvent::PluginResultProduced),
            Payload::Plugin(PluginEventPayload { plugin_id }),
        )
    }
}

/// Trait that plugin authors implement to handle events from the daemon.
///
/// All handler methods have default no-op implementations, so plugins only
/// need to override the events they care about.
pub trait Plugin {
    /// The plugin's name (used for logging).
    fn name(&self) -> &str;

    /// Numeric plugin ID used in event payloads. Defaults to 0.
    fn plugin_id(&self) -> i32 {
        0
    }

    /// Called once when the runtime starts, before the event loop.
    fn on_start(&self, _ctx: &EventContext) -> Result<()> {
        Ok(())
    }

    /// Called once when the runtime is shutting down.
    fn on_stop(&self, _ctx: &EventContext) -> Result<()> {
        Ok(())
    }

    /// Called when a task event is received from the daemon.
    fn on_task_event(
        &self,
        _event: TaskEvent,
        _payload: TaskEventPayload,
        _ctx: &EventContext,
    ) -> Result<()> {
        Ok(())
    }

    /// Called when a plugin event is received.
    fn on_plugin_event(
        &self,
        _event: PluginEvent,
        _payload: PluginEventPayload,
        _ctx: &EventContext,
    ) -> Result<()> {
        Ok(())
    }

    /// Called when a sample event is received.
    fn on_sample_event(
        &self,
        _event: SampleEvent,
        _payload: SampleEventPayload,
        _ctx: &EventContext,
    ) -> Result<()> {
        Ok(())
    }

    /// Called when a daemon event is received (shutdown, config reload).
    fn on_daemon_event(&self, _event: DaemonEvent, _ctx: &EventContext) -> Result<()> {
        Ok(())
    }
}
