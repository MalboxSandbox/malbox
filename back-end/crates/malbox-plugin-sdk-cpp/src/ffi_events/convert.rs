//! Rust <-> C conversion functions for the flat event type.

use super::types::*;
use malbox_plugin_transport::messages::events::Event;

/// Convert a Rust `Event` to its C-ABI representation.
pub(crate) fn rust_event_to_c(event: &Event) -> MalboxEvent {
    match event {
        Event::TaskCreated { task_id } => MalboxEvent {
            tag: MalboxEventTag::TaskCreated,
            id: *task_id,
        },
        Event::TaskStarting { task_id } => MalboxEvent {
            tag: MalboxEventTag::TaskStarting,
            id: *task_id,
        },
        Event::TaskCompleted { task_id } => MalboxEvent {
            tag: MalboxEventTag::TaskCompleted,
            id: *task_id,
        },
        Event::TaskFailed { task_id } => MalboxEvent {
            tag: MalboxEventTag::TaskFailed,
            id: *task_id,
        },
        Event::TaskCanceled { task_id } => MalboxEvent {
            tag: MalboxEventTag::TaskCanceled,
            id: *task_id,
        },
        Event::PluginStarted { plugin_id } => MalboxEvent {
            tag: MalboxEventTag::PluginStarted,
            id: *plugin_id,
        },
        Event::PluginStopped { plugin_id } => MalboxEvent {
            tag: MalboxEventTag::PluginStopped,
            id: *plugin_id,
        },
        Event::PluginResultProduced { plugin_id } => MalboxEvent {
            tag: MalboxEventTag::PluginResultProduced,
            id: *plugin_id,
        },
        Event::SampleStarted { sample_id } => MalboxEvent {
            tag: MalboxEventTag::SampleStarted,
            id: *sample_id,
        },
        Event::SampleStopped { sample_id } => MalboxEvent {
            tag: MalboxEventTag::SampleStopped,
            id: *sample_id,
        },
        Event::SampleResultProduced { sample_id } => MalboxEvent {
            tag: MalboxEventTag::SampleResultProduced,
            id: *sample_id,
        },
        Event::DaemonShutdown => MalboxEvent {
            tag: MalboxEventTag::DaemonShutdown,
            id: 0,
        },
        Event::ConfigReloaded => MalboxEvent {
            tag: MalboxEventTag::ConfigReloaded,
            id: 0,
        },
    }
}

/// Convert a C-ABI `MalboxEvent` back to a Rust `Event`.
///
/// Returns `Err(())` if the tag value is unrecognised.
pub(crate) fn c_event_to_rust(event: &MalboxEvent) -> Result<Event, ()> {
    Ok(match event.tag {
        MalboxEventTag::TaskCreated => Event::TaskCreated { task_id: event.id },
        MalboxEventTag::TaskStarting => Event::TaskStarting { task_id: event.id },
        MalboxEventTag::TaskCompleted => Event::TaskCompleted { task_id: event.id },
        MalboxEventTag::TaskFailed => Event::TaskFailed { task_id: event.id },
        MalboxEventTag::TaskCanceled => Event::TaskCanceled { task_id: event.id },
        MalboxEventTag::PluginStarted => Event::PluginStarted {
            plugin_id: event.id,
        },
        MalboxEventTag::PluginStopped => Event::PluginStopped {
            plugin_id: event.id,
        },
        MalboxEventTag::PluginResultProduced => Event::PluginResultProduced {
            plugin_id: event.id,
        },
        MalboxEventTag::SampleStarted => Event::SampleStarted {
            sample_id: event.id,
        },
        MalboxEventTag::SampleStopped => Event::SampleStopped {
            sample_id: event.id,
        },
        MalboxEventTag::SampleResultProduced => Event::SampleResultProduced {
            sample_id: event.id,
        },
        MalboxEventTag::DaemonShutdown => Event::DaemonShutdown,
        MalboxEventTag::ConfigReloaded => Event::ConfigReloaded,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_roundtrip(event: Event) {
        let c = rust_event_to_c(&event);
        let back = c_event_to_rust(&c).expect("round-trip failed");
        assert_eq!(event, back);
    }

    #[test]
    fn roundtrip_task_created() {
        event_roundtrip(Event::TaskCreated { task_id: 7 });
    }

    #[test]
    fn roundtrip_task_starting() {
        event_roundtrip(Event::TaskStarting { task_id: 42 });
    }

    #[test]
    fn roundtrip_task_completed() {
        event_roundtrip(Event::TaskCompleted { task_id: 1 });
    }

    #[test]
    fn roundtrip_task_failed() {
        event_roundtrip(Event::TaskFailed { task_id: 0 });
    }

    #[test]
    fn roundtrip_task_canceled() {
        event_roundtrip(Event::TaskCanceled { task_id: 99 });
    }

    #[test]
    fn roundtrip_plugin_started() {
        event_roundtrip(Event::PluginStarted { plugin_id: 3 });
    }

    #[test]
    fn roundtrip_plugin_stopped() {
        event_roundtrip(Event::PluginStopped { plugin_id: 5 });
    }

    #[test]
    fn roundtrip_plugin_result_produced() {
        event_roundtrip(Event::PluginResultProduced { plugin_id: 99 });
    }

    #[test]
    fn roundtrip_sample_started() {
        event_roundtrip(Event::SampleStarted { sample_id: 10 });
    }

    #[test]
    fn roundtrip_sample_stopped() {
        event_roundtrip(Event::SampleStopped { sample_id: 20 });
    }

    #[test]
    fn roundtrip_sample_result_produced() {
        event_roundtrip(Event::SampleResultProduced { sample_id: 30 });
    }

    #[test]
    fn roundtrip_daemon_shutdown() {
        event_roundtrip(Event::DaemonShutdown);
    }

    #[test]
    fn roundtrip_daemon_config_reloaded() {
        event_roundtrip(Event::ConfigReloaded);
    }

    #[test]
    fn daemon_events_have_zero_id() {
        let c = rust_event_to_c(&Event::DaemonShutdown);
        assert_eq!(c.id, 0);
        let c = rust_event_to_c(&Event::ConfigReloaded);
        assert_eq!(c.id, 0);
    }
}
