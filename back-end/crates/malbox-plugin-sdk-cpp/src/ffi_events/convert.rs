//! Rust <-> C conversion functions for the flat event type.

use super::types::*;
use malbox_plugin_transport::messages::events::Event;
use std::ffi::CString;

/// Helper to create a MalboxEvent with null string pointers (most events).
fn simple_event(tag: MalboxEventTag, id: i32) -> MalboxEvent {
    MalboxEvent {
        tag,
        id,
        source: std::ptr::null(),
        result_name: std::ptr::null(),
    }
}

/// An owned C event that keeps CStrings alive for `PluginResultAvailable`.
///
/// The `event` field contains raw pointers into `_source` and `_result_name`.
/// Drop this struct only after the event has been consumed by the callback.
pub(crate) struct OwnedCEvent {
    pub event: MalboxEvent,
    _source: Option<CString>,
    _result_name: Option<CString>,
}

/// Convert a Rust `Event` to its C-ABI representation.
///
/// For `PluginResultAvailable`, the returned `OwnedCEvent` owns the string
/// data that `event.source` and `event.result_name` point into. The caller
/// must keep the `OwnedCEvent` alive until the callback returns.
pub(crate) fn rust_event_to_c(event: &Event) -> OwnedCEvent {
    match event {
        Event::TaskCreated { task_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::TaskCreated, *task_id),
            _source: None,
            _result_name: None,
        },
        Event::TaskStarting { task_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::TaskStarting, *task_id),
            _source: None,
            _result_name: None,
        },
        Event::TaskCompleted { task_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::TaskCompleted, *task_id),
            _source: None,
            _result_name: None,
        },
        Event::TaskFailed { task_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::TaskFailed, *task_id),
            _source: None,
            _result_name: None,
        },
        Event::TaskCanceled { task_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::TaskCanceled, *task_id),
            _source: None,
            _result_name: None,
        },
        Event::PluginStarted { plugin_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::PluginStarted, *plugin_id),
            _source: None,
            _result_name: None,
        },
        Event::PluginStopped { plugin_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::PluginStopped, *plugin_id),
            _source: None,
            _result_name: None,
        },
        Event::PluginResultAvailable {
            source,
            result_name,
        } => {
            let source_cs =
                CString::new(source.as_str()).unwrap_or_else(|_| CString::new("").unwrap());
            let result_name_cs =
                CString::new(result_name.as_str()).unwrap_or_else(|_| CString::new("").unwrap());
            let event = MalboxEvent {
                tag: MalboxEventTag::PluginResultAvailable,
                id: 0,
                source: source_cs.as_ptr(),
                result_name: result_name_cs.as_ptr(),
            };
            OwnedCEvent {
                event,
                _source: Some(source_cs),
                _result_name: Some(result_name_cs),
            }
        }
        Event::SampleStarted { sample_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::SampleStarted, *sample_id),
            _source: None,
            _result_name: None,
        },
        Event::SampleStopped { sample_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::SampleStopped, *sample_id),
            _source: None,
            _result_name: None,
        },
        Event::SampleResultProduced { sample_id } => OwnedCEvent {
            event: simple_event(MalboxEventTag::SampleResultProduced, *sample_id),
            _source: None,
            _result_name: None,
        },
        Event::DaemonShutdown => OwnedCEvent {
            event: simple_event(MalboxEventTag::DaemonShutdown, 0),
            _source: None,
            _result_name: None,
        },
        Event::ConfigReloaded => OwnedCEvent {
            event: simple_event(MalboxEventTag::ConfigReloaded, 0),
            _source: None,
            _result_name: None,
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
        MalboxEventTag::PluginResultAvailable => {
            let source = if event.source.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(event.source) }
                    .to_str()
                    .unwrap_or("")
                    .to_owned()
            };
            let result_name = if event.result_name.is_null() {
                String::new()
            } else {
                unsafe { std::ffi::CStr::from_ptr(event.result_name) }
                    .to_str()
                    .unwrap_or("")
                    .to_owned()
            };
            Event::PluginResultAvailable {
                source,
                result_name,
            }
        }
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
        let owned = rust_event_to_c(&event);
        let back = c_event_to_rust(&owned.event).expect("round-trip failed");
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
    fn roundtrip_plugin_result_available() {
        event_roundtrip(Event::PluginResultAvailable {
            source: String::new(),
            result_name: String::new(),
        });
    }

    #[test]
    fn roundtrip_plugin_result_available_with_data() {
        event_roundtrip(Event::PluginResultAvailable {
            source: "host-yara".to_string(),
            result_name: "yara_matches".to_string(),
        });
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
        let owned = rust_event_to_c(&Event::DaemonShutdown);
        assert_eq!(owned.event.id, 0);
        let owned = rust_event_to_c(&Event::ConfigReloaded);
        assert_eq!(owned.event.id, 0);
    }

    #[test]
    fn simple_events_have_null_strings() {
        let owned = rust_event_to_c(&Event::TaskCreated { task_id: 1 });
        assert!(owned.event.source.is_null());
        assert!(owned.event.result_name.is_null());
    }

    #[test]
    fn result_available_has_non_null_strings() {
        let owned = rust_event_to_c(&Event::PluginResultAvailable {
            source: "src".to_string(),
            result_name: "res".to_string(),
        });
        assert!(!owned.event.source.is_null());
        assert!(!owned.event.result_name.is_null());
        let src = unsafe { std::ffi::CStr::from_ptr(owned.event.source) }
            .to_str()
            .unwrap();
        let rn = unsafe { std::ffi::CStr::from_ptr(owned.event.result_name) }
            .to_str()
            .unwrap();
        assert_eq!(src, "src");
        assert_eq!(rn, "res");
    }
}
