//! `tracing_subscriber::Layer` implementation that captures events into a
//! [`LogBus`](super::LogBus).

use super::{LogBus, LogEntry, LogLevel};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;

/// A [`tracing_subscriber::Layer`] that captures events into a [`LogBus`].
pub struct GuestLogLayer {
    bus: Arc<LogBus>,
}

impl GuestLogLayer {
    /// Create a new layer that pushes events into `bus`.
    pub(super) fn new(bus: Arc<LogBus>) -> Self {
        Self { bus }
    }
}

impl<S: Subscriber> Layer<S> for GuestLogLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let meta = event.metadata();

        let timestamp_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let level = LogLevel::from(meta.level());
        let target = meta.target().to_string();

        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);

        let entry = LogEntry {
            timestamp_ns,
            level,
            target,
            message: visitor.message,
            fields: visitor.fields,
        };

        self.bus.push(entry);
    }
}

/// Visitor that extracts the `message` field specially and collects all other
/// fields as string key-value pairs.
#[derive(Default)]
struct FieldVisitor {
    message: String,
    fields: HashMap<String, String>,
}

impl FieldVisitor {
    fn record_field(&mut self, field: &Field, value: String) {
        if field.name() == "message" {
            self.message = value;
        } else {
            self.fields.insert(field.name().to_string(), value);
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.record_field(field, format!("{:?}", value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.record_field(field, value.to_string());
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.record_field(field, value.to_string());
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.record_field(field, value.to_string());
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.record_field(field, value.to_string());
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.record_field(field, value.to_string());
    }
}
