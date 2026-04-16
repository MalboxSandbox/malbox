//! Proto <-> Event conversions.
//!
//! Bridges the prost-generated protobuf types (`super::proto`) and the
//! transport-agnostic internal types (`crate::messages::events`).

use super::proto;
use crate::error::{Result, TransportError};
use crate::messages::events::Event;

/// Convert an internal flat `Event` to a proto `EventNotification`.
pub fn event_to_proto(event: &Event) -> Result<proto::EventNotification> {
    let proto_event = match event {
        Event::TaskCreated { task_id } => {
            proto::event_notification::Event::Task(proto::TaskEventProto {
                event_type: proto::TaskEventType::Created.into(),
                task_id: *task_id,
            })
        }
        Event::TaskStarting { task_id } => {
            proto::event_notification::Event::Task(proto::TaskEventProto {
                event_type: proto::TaskEventType::Starting.into(),
                task_id: *task_id,
            })
        }
        Event::TaskCompleted { task_id } => {
            proto::event_notification::Event::Task(proto::TaskEventProto {
                event_type: proto::TaskEventType::Completed.into(),
                task_id: *task_id,
            })
        }
        Event::TaskFailed { task_id } => {
            proto::event_notification::Event::Task(proto::TaskEventProto {
                event_type: proto::TaskEventType::Failed.into(),
                task_id: *task_id,
            })
        }
        Event::TaskCanceled { task_id } => {
            proto::event_notification::Event::Task(proto::TaskEventProto {
                event_type: proto::TaskEventType::Canceled.into(),
                task_id: *task_id,
            })
        }
        Event::PluginStarted { plugin_id } => {
            proto::event_notification::Event::Plugin(proto::PluginEventProto {
                event_type: proto::PluginEventType::Started.into(),
                plugin_id: *plugin_id,
            })
        }
        Event::PluginStopped { plugin_id } => {
            proto::event_notification::Event::Plugin(proto::PluginEventProto {
                event_type: proto::PluginEventType::Stopped.into(),
                plugin_id: *plugin_id,
            })
        }
        Event::PluginResultProduced { plugin_id } => {
            proto::event_notification::Event::Plugin(proto::PluginEventProto {
                event_type: proto::PluginEventType::ResultProduced.into(),
                plugin_id: *plugin_id,
            })
        }
        Event::SampleStarted { sample_id } => {
            proto::event_notification::Event::Sample(proto::SampleEventProto {
                event_type: proto::SampleEventType::Started.into(),
                sample_id: *sample_id,
            })
        }
        Event::SampleStopped { sample_id } => {
            proto::event_notification::Event::Sample(proto::SampleEventProto {
                event_type: proto::SampleEventType::Stopped.into(),
                sample_id: *sample_id,
            })
        }
        Event::SampleResultProduced { sample_id } => {
            proto::event_notification::Event::Sample(proto::SampleEventProto {
                event_type: proto::SampleEventType::ResultProduced.into(),
                sample_id: *sample_id,
            })
        }
        Event::DaemonShutdown => {
            proto::event_notification::Event::Daemon(proto::DaemonEventProto {
                event_type: proto::DaemonEventType::Shutdown.into(),
            })
        }
        Event::ConfigReloaded => {
            proto::event_notification::Event::Daemon(proto::DaemonEventProto {
                event_type: proto::DaemonEventType::ConfigReloaded.into(),
            })
        }
    };

    Ok(proto::EventNotification {
        event: Some(proto_event),
    })
}

/// Convert a proto `EventNotification` back to an internal `Event`.
pub fn proto_to_event(notification: proto::EventNotification) -> Result<Event> {
    let inner = notification
        .event
        .ok_or_else(|| TransportError::Grpc("EventNotification has no event set".into()))?;

    match inner {
        proto::event_notification::Event::Task(task_proto) => {
            match proto::TaskEventType::try_from(task_proto.event_type) {
                Ok(proto::TaskEventType::Created) => Ok(Event::TaskCreated {
                    task_id: task_proto.task_id,
                }),
                Ok(proto::TaskEventType::Starting) => Ok(Event::TaskStarting {
                    task_id: task_proto.task_id,
                }),
                Ok(proto::TaskEventType::Completed) => Ok(Event::TaskCompleted {
                    task_id: task_proto.task_id,
                }),
                Ok(proto::TaskEventType::Failed) => Ok(Event::TaskFailed {
                    task_id: task_proto.task_id,
                }),
                Ok(proto::TaskEventType::Canceled) => Ok(Event::TaskCanceled {
                    task_id: task_proto.task_id,
                }),
                Ok(proto::TaskEventType::Unspecified) | Err(_) => Err(TransportError::Grpc(
                    format!("unknown TaskEventType value: {}", task_proto.event_type),
                )),
            }
        }
        proto::event_notification::Event::Plugin(plugin_proto) => {
            match proto::PluginEventType::try_from(plugin_proto.event_type) {
                Ok(proto::PluginEventType::Started) => Ok(Event::PluginStarted {
                    plugin_id: plugin_proto.plugin_id,
                }),
                Ok(proto::PluginEventType::Stopped) => Ok(Event::PluginStopped {
                    plugin_id: plugin_proto.plugin_id,
                }),
                Ok(proto::PluginEventType::ResultProduced) => Ok(Event::PluginResultProduced {
                    plugin_id: plugin_proto.plugin_id,
                }),
                Ok(proto::PluginEventType::Unspecified) | Err(_) => Err(TransportError::Grpc(
                    format!("unknown PluginEventType value: {}", plugin_proto.event_type),
                )),
            }
        }
        proto::event_notification::Event::Sample(sample_proto) => {
            match proto::SampleEventType::try_from(sample_proto.event_type) {
                Ok(proto::SampleEventType::Started) => Ok(Event::SampleStarted {
                    sample_id: sample_proto.sample_id,
                }),
                Ok(proto::SampleEventType::Stopped) => Ok(Event::SampleStopped {
                    sample_id: sample_proto.sample_id,
                }),
                Ok(proto::SampleEventType::ResultProduced) => Ok(Event::SampleResultProduced {
                    sample_id: sample_proto.sample_id,
                }),
                Ok(proto::SampleEventType::Unspecified) | Err(_) => Err(TransportError::Grpc(
                    format!("unknown SampleEventType value: {}", sample_proto.event_type),
                )),
            }
        }
        proto::event_notification::Event::Daemon(daemon_proto) => {
            match proto::DaemonEventType::try_from(daemon_proto.event_type) {
                Ok(proto::DaemonEventType::Shutdown) => Ok(Event::DaemonShutdown),
                Ok(proto::DaemonEventType::ConfigReloaded) => Ok(Event::ConfigReloaded),
                Ok(proto::DaemonEventType::Unspecified) | Err(_) => Err(TransportError::Grpc(
                    format!("unknown DaemonEventType value: {}", daemon_proto.event_type),
                )),
            }
        }
    }
}

/// Convert an internal `Event` into a `TaskResult` proto message.
///
/// This is used by `GrpcEmitter` to send event metadata back through the
/// `ExecuteTask` streaming response. The event description is placed in
/// `result_name` and format is set to JSON (metadata, not binary data).
pub fn event_to_task_result(task_id: i32, event: &Event) -> proto::TaskResult {
    let result_name = format!("{:?}", event);

    let data = match event {
        Event::TaskCreated { task_id }
        | Event::TaskStarting { task_id }
        | Event::TaskCompleted { task_id }
        | Event::TaskFailed { task_id }
        | Event::TaskCanceled { task_id } => format!(r#"{{"task_id":{}}}"#, task_id).into_bytes(),
        Event::PluginStarted { plugin_id }
        | Event::PluginStopped { plugin_id }
        | Event::PluginResultProduced { plugin_id } => {
            format!(r#"{{"plugin_id":{}}}"#, plugin_id).into_bytes()
        }
        Event::SampleStarted { sample_id }
        | Event::SampleStopped { sample_id }
        | Event::SampleResultProduced { sample_id } => {
            format!(r#"{{"sample_id":{}}}"#, sample_id).into_bytes()
        }
        Event::DaemonShutdown | Event::ConfigReloaded => b"{}".to_vec(),
    };

    proto::TaskResult {
        task_id,
        result_name,
        data,
        format: proto::ResultFormat::Json.into(),
        is_final: false,
        kind: proto::ResultKind::Result.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Task roundtrip tests ────────────────────────────────────────

    #[test]
    fn test_task_created_roundtrip() {
        let event = Event::TaskCreated { task_id: 42 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    #[test]
    fn test_task_starting_roundtrip() {
        let event = Event::TaskStarting { task_id: 10 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    #[test]
    fn test_task_completed_roundtrip() {
        let event = Event::TaskCompleted { task_id: 55 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    #[test]
    fn test_task_failed_roundtrip() {
        let event = Event::TaskFailed { task_id: 7 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    // ── Plugin roundtrip tests ──────────────────────────────────────

    #[test]
    fn test_plugin_started_roundtrip() {
        let event = Event::PluginStarted { plugin_id: 3 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    #[test]
    fn test_plugin_stopped_roundtrip() {
        let event = Event::PluginStopped { plugin_id: 15 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    #[test]
    fn test_plugin_result_produced_roundtrip() {
        let event = Event::PluginResultProduced { plugin_id: 8 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    // ── Sample roundtrip tests ──────────────────────────────────────

    #[test]
    fn test_sample_started_roundtrip() {
        let event = Event::SampleStarted { sample_id: 99 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    #[test]
    fn test_sample_stopped_roundtrip() {
        let event = Event::SampleStopped { sample_id: 33 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    #[test]
    fn test_sample_result_produced_roundtrip() {
        let event = Event::SampleResultProduced { sample_id: 77 };
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    // ── Daemon roundtrip tests ──────────────────────────────────────

    #[test]
    fn test_daemon_shutdown_roundtrip() {
        let event = Event::DaemonShutdown;
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    #[test]
    fn test_config_reloaded_roundtrip() {
        let event = Event::ConfigReloaded;
        let proto_notif = event_to_proto(&event).expect("event_to_proto should succeed");
        let event_back = proto_to_event(proto_notif).expect("proto_to_event should succeed");
        assert_eq!(event, event_back);
    }

    // ── Error tests ─────────────────────────────────────────────────

    #[test]
    fn test_empty_notification_fails() {
        let notification = proto::EventNotification { event: None };
        let result = proto_to_event(notification);
        assert!(
            result.is_err(),
            "empty notification should produce an error"
        );
    }

    // ── event_to_task_result tests ──────────────────────────────────

    #[test]
    fn test_event_to_task_result_task() {
        let event = Event::TaskCreated { task_id: 42 };
        let result = event_to_task_result(100, &event);
        assert_eq!(result.task_id, 100);
        assert!(result.result_name.contains("TaskCreated"));
        assert_eq!(result.data, br#"{"task_id":42}"#);
        assert_eq!(result.format, i32::from(proto::ResultFormat::Json));
        assert!(!result.is_final);
    }

    #[test]
    fn test_event_to_task_result_plugin() {
        let event = Event::PluginStarted { plugin_id: 5 };
        let result = event_to_task_result(200, &event);
        assert_eq!(result.task_id, 200);
        assert!(result.result_name.contains("PluginStarted"));
        assert_eq!(result.data, br#"{"plugin_id":5}"#);
    }

    #[test]
    fn test_event_to_task_result_sample() {
        let event = Event::SampleStopped { sample_id: 11 };
        let result = event_to_task_result(300, &event);
        assert_eq!(result.task_id, 300);
        assert!(result.result_name.contains("SampleStopped"));
        assert_eq!(result.data, br#"{"sample_id":11}"#);
    }

    #[test]
    fn test_event_to_task_result_daemon() {
        let event = Event::DaemonShutdown;
        let result = event_to_task_result(400, &event);
        assert_eq!(result.task_id, 400);
        assert!(result.result_name.contains("DaemonShutdown"));
        assert_eq!(result.data, b"{}");
    }
}
