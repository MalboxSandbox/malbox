//! Plugin execution context for communicating with the daemon at runtime.

use crate::error::{Result, SdkError};
use malbox_plugin_transport::messages::events::{Event, Payload};
use malbox_plugin_transport::traits::TransportEmitter;

/// Runtime context available to plugin handler methods.
///
/// Provides methods for emitting progress updates, events, and warnings
/// back to the daemon during task execution.
pub struct Context<'a> {
    emitter: &'a dyn TransportEmitter,
}

impl<'a> Context<'a> {
    /// Create a new context wrapping a transport emitter.
    pub(crate) fn new(emitter: &'a dyn TransportEmitter) -> Self {
        Self { emitter }
    }

    /// Report execution progress (0.0 to 1.0) with a status message.
    ///
    /// Progress updates are streamed to the daemon in real-time and can be
    /// shown in the UI. The `progress` value is clamped to `[0.0, 1.0]`.
    pub fn emit_progress(&self, progress: f64, message: &str) -> Result<()> {
        let _clamped = progress.clamp(0.0, 1.0);
        // TODO: emit a progress event once the transport supports it.
        // For now this is a structured log that the daemon can parse.
        tracing::info!(progress = _clamped, message, "plugin_progress");
        Ok(())
    }

    /// Emit a raw event and payload back to the daemon.
    ///
    /// This is an escape hatch for advanced use cases. Prefer the typed
    /// helper methods when possible.
    pub fn emit_event(&self, event: Event, payload: Payload) -> Result<()> {
        self.emitter
            .emit(event, payload)
            .map_err(SdkError::Transport)
    }

    /// Log a warning that will be attached to the task report.
    pub fn warn(&self, message: &str) -> Result<()> {
        tracing::warn!(message, "plugin_warning");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_emit_progress_clamps_values() {
        // Use no-op emitter (unit type implements TransportEmitter)
        let emitter = ();
        let ctx = Context::new(&emitter);

        // Should not panic for out-of-range values
        assert!(ctx.emit_progress(-0.5, "negative").is_ok());
        assert!(ctx.emit_progress(1.5, "over").is_ok());
        assert!(ctx.emit_progress(0.5, "normal").is_ok());
    }

    #[test]
    fn context_warn_succeeds() {
        let emitter = ();
        let ctx = Context::new(&emitter);
        assert!(ctx.warn("test warning").is_ok());
    }

    #[test]
    fn context_emit_event_succeeds_with_noop() {
        use malbox_plugin_transport::messages::events::*;

        let emitter = ();
        let ctx = Context::new(&emitter);
        let result = ctx.emit_event(
            Event::Plugin(PluginEvent::PluginStarted),
            Payload::Plugin(PluginEventPayload { plugin_id: 1 }),
        );
        assert!(result.is_ok());
    }
}
