//! Proto <-> Event/Payload conversions.
//!
//! Bridges the prost-generated protobuf types (`super::proto`) and the
//! transport-agnostic internal types (`crate::messages::events`).

use super::proto;
use crate::error::{Result, TransportError};
use crate::messages::events::*;

/// Convert internal `Event` + `Payload` to a proto `EventNotification`.
pub fn event_to_proto(event: &Event, payload: &Payload) -> Result<proto::EventNotification> {
    let proto_event = match event {
        Event::Task(task_evt) => {
            let task_id = match payload {
                Payload::Task(tp) => tp.task_id,
                _ => 0,
            };
            let event_type: i32 = match task_evt {
                TaskEvent::TaskCreated => proto::TaskEventType::Created.into(),
                TaskEvent::TaskStarting => proto::TaskEventType::Starting.into(),
                TaskEvent::TaskCompleted => proto::TaskEventType::Completed.into(),
                TaskEvent::TaskFailed => proto::TaskEventType::Failed.into(),
            };
            proto::event_notification::Event::Task(proto::TaskEventProto {
                event_type,
                task_id,
            })
        }
        Event::Plugin(plugin_evt) => {
            let plugin_id = match payload {
                Payload::Plugin(pp) => pp.plugin_id,
                _ => 0,
            };
            let event_type: i32 = match plugin_evt {
                PluginEvent::PluginStarted => proto::PluginEventType::Started.into(),
                PluginEvent::PluginStopped => proto::PluginEventType::Stopped.into(),
                PluginEvent::PluginResultProduced => proto::PluginEventType::ResultProduced.into(),
            };
            proto::event_notification::Event::Plugin(proto::PluginEventProto {
                event_type,
                plugin_id,
            })
        }
        Event::Sample(sample_evt) => {
            let sample_id = match payload {
                Payload::Sample(sp) => sp.sample_id,
                _ => 0,
            };
            let event_type: i32 = match sample_evt {
                SampleEvent::PluginStarted => proto::SampleEventType::Started.into(),
                SampleEvent::PluginStopped => proto::SampleEventType::Stopped.into(),
                SampleEvent::PluginResultProduced => proto::SampleEventType::ResultProduced.into(),
            };
            proto::event_notification::Event::Sample(proto::SampleEventProto {
                event_type,
                sample_id,
            })
        }
        Event::Daemon(daemon_evt) => {
            let event_type: i32 = match daemon_evt {
                DaemonEvent::DaemonShutdown => proto::DaemonEventType::Shutdown.into(),
                DaemonEvent::ConfigReloaded => proto::DaemonEventType::ConfigReloaded.into(),
            };
            proto::event_notification::Event::Daemon(proto::DaemonEventProto { event_type })
        }
    };

    Ok(proto::EventNotification {
        event: Some(proto_event),
    })
}

/// Convert a proto `EventNotification` back to internal `(Event, Payload)`.
pub fn proto_to_event(notification: proto::EventNotification) -> Result<(Event, Payload)> {
    let inner = notification
        .event
        .ok_or_else(|| TransportError::Grpc("EventNotification has no event set".into()))?;

    match inner {
        proto::event_notification::Event::Task(task_proto) => {
            let task_event = match proto::TaskEventType::try_from(task_proto.event_type) {
                Ok(proto::TaskEventType::Created) => TaskEvent::TaskCreated,
                Ok(proto::TaskEventType::Starting) => TaskEvent::TaskStarting,
                Ok(proto::TaskEventType::Completed) => TaskEvent::TaskCompleted,
                Ok(proto::TaskEventType::Failed) => TaskEvent::TaskFailed,
                Ok(proto::TaskEventType::Unspecified) | Err(_) => {
                    return Err(TransportError::Grpc(format!(
                        "unknown TaskEventType value: {}",
                        task_proto.event_type
                    )));
                }
            };
            Ok((
                Event::Task(task_event),
                Payload::Task(TaskEventPayload {
                    task_id: task_proto.task_id,
                }),
            ))
        }
        proto::event_notification::Event::Plugin(plugin_proto) => {
            let plugin_event = match proto::PluginEventType::try_from(plugin_proto.event_type) {
                Ok(proto::PluginEventType::Started) => PluginEvent::PluginStarted,
                Ok(proto::PluginEventType::Stopped) => PluginEvent::PluginStopped,
                Ok(proto::PluginEventType::ResultProduced) => PluginEvent::PluginResultProduced,
                Ok(proto::PluginEventType::Unspecified) | Err(_) => {
                    return Err(TransportError::Grpc(format!(
                        "unknown PluginEventType value: {}",
                        plugin_proto.event_type
                    )));
                }
            };
            Ok((
                Event::Plugin(plugin_event),
                Payload::Plugin(PluginEventPayload {
                    plugin_id: plugin_proto.plugin_id,
                }),
            ))
        }
        proto::event_notification::Event::Sample(sample_proto) => {
            let sample_event = match proto::SampleEventType::try_from(sample_proto.event_type) {
                Ok(proto::SampleEventType::Started) => SampleEvent::PluginStarted,
                Ok(proto::SampleEventType::Stopped) => SampleEvent::PluginStopped,
                Ok(proto::SampleEventType::ResultProduced) => SampleEvent::PluginResultProduced,
                Ok(proto::SampleEventType::Unspecified) | Err(_) => {
                    return Err(TransportError::Grpc(format!(
                        "unknown SampleEventType value: {}",
                        sample_proto.event_type
                    )));
                }
            };
            Ok((
                Event::Sample(sample_event),
                Payload::Sample(SampleEventPayload {
                    sample_id: sample_proto.sample_id,
                }),
            ))
        }
        proto::event_notification::Event::Daemon(daemon_proto) => {
            let daemon_event = match proto::DaemonEventType::try_from(daemon_proto.event_type) {
                Ok(proto::DaemonEventType::Shutdown) => DaemonEvent::DaemonShutdown,
                Ok(proto::DaemonEventType::ConfigReloaded) => DaemonEvent::ConfigReloaded,
                Ok(proto::DaemonEventType::Unspecified) | Err(_) => {
                    return Err(TransportError::Grpc(format!(
                        "unknown DaemonEventType value: {}",
                        daemon_proto.event_type
                    )));
                }
            };
            // Daemon events have no matching payload enum -- use Task placeholder
            Ok((
                Event::Daemon(daemon_event),
                Payload::Task(TaskEventPayload { task_id: 0 }),
            ))
        }
    }
}

/// Convert an internal `Event` + `Payload` into a `TaskResult` proto message.
///
/// This is used by `GrpcEmitter` to send event metadata back through the
/// `ExecuteTask` streaming response. The event description is placed in
/// `result_name` and format is set to JSON (metadata, not binary data).
pub fn event_to_task_result(task_id: i32, event: &Event, payload: &Payload) -> proto::TaskResult {
    let result_name = match event {
        Event::Task(t) => format!("task:{:?}", t),
        Event::Plugin(p) => format!("plugin:{:?}", p),
        Event::Sample(s) => format!("sample:{:?}", s),
        Event::Daemon(d) => format!("daemon:{:?}", d),
    };

    let data = match payload {
        Payload::Task(tp) => format!(r#"{{"task_id":{}}}"#, tp.task_id).into_bytes(),
        Payload::Plugin(pp) => format!(r#"{{"plugin_id":{}}}"#, pp.plugin_id).into_bytes(),
        Payload::Sample(sp) => format!(r#"{{"sample_id":{}}}"#, sp.sample_id).into_bytes(),
    };

    proto::TaskResult {
        task_id,
        result_name,
        data,
        format: proto::ResultFormat::Json.into(),
        is_final: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_created_roundtrip() {
        let event = Event::Task(TaskEvent::TaskCreated);
        let payload = Payload::Task(TaskEventPayload { task_id: 42 });

        let proto_notif = event_to_proto(&event, &payload).expect("event_to_proto should succeed");
        let (event_back, payload_back) =
            proto_to_event(proto_notif).expect("proto_to_event should succeed");

        match event_back {
            Event::Task(TaskEvent::TaskCreated) => {}
            other => panic!("expected TaskCreated, got {:?}", other),
        }
        match payload_back {
            Payload::Task(ref tp) => assert_eq!(tp.task_id, 42),
            other => panic!("expected Task payload, got {:?}", other),
        }
    }

    #[test]
    fn test_task_failed_roundtrip() {
        let event = Event::Task(TaskEvent::TaskFailed);
        let payload = Payload::Task(TaskEventPayload { task_id: 7 });

        let proto_notif = event_to_proto(&event, &payload).expect("event_to_proto should succeed");
        let (event_back, payload_back) =
            proto_to_event(proto_notif).expect("proto_to_event should succeed");

        match event_back {
            Event::Task(TaskEvent::TaskFailed) => {}
            other => panic!("expected TaskFailed, got {:?}", other),
        }
        match payload_back {
            Payload::Task(ref tp) => assert_eq!(tp.task_id, 7),
            other => panic!("expected Task payload, got {:?}", other),
        }
    }

    #[test]
    fn test_plugin_started_roundtrip() {
        let event = Event::Plugin(PluginEvent::PluginStarted);
        let payload = Payload::Plugin(PluginEventPayload { plugin_id: 3 });

        let proto_notif = event_to_proto(&event, &payload).expect("event_to_proto should succeed");
        let (event_back, payload_back) =
            proto_to_event(proto_notif).expect("proto_to_event should succeed");

        match event_back {
            Event::Plugin(PluginEvent::PluginStarted) => {}
            other => panic!("expected PluginStarted, got {:?}", other),
        }
        match payload_back {
            Payload::Plugin(ref pp) => assert_eq!(pp.plugin_id, 3),
            other => panic!("expected Plugin payload, got {:?}", other),
        }
    }

    #[test]
    fn test_daemon_shutdown_roundtrip() {
        // Daemon events have no dedicated payload; use Task placeholder with task_id: 0
        let event = Event::Daemon(DaemonEvent::DaemonShutdown);
        let payload = Payload::Task(TaskEventPayload { task_id: 0 });

        let proto_notif = event_to_proto(&event, &payload).expect("event_to_proto should succeed");
        let (event_back, payload_back) =
            proto_to_event(proto_notif).expect("proto_to_event should succeed");

        match event_back {
            Event::Daemon(DaemonEvent::DaemonShutdown) => {}
            other => panic!("expected DaemonShutdown, got {:?}", other),
        }
        // Daemon round-trips produce placeholder Task payload
        match payload_back {
            Payload::Task(ref tp) => assert_eq!(tp.task_id, 0),
            other => panic!("expected placeholder Task payload, got {:?}", other),
        }
    }

    #[test]
    fn test_sample_event_roundtrip() {
        let event = Event::Sample(SampleEvent::PluginStarted);
        let payload = Payload::Sample(SampleEventPayload { sample_id: 99 });

        let proto_notif = event_to_proto(&event, &payload).expect("event_to_proto should succeed");
        let (event_back, payload_back) =
            proto_to_event(proto_notif).expect("proto_to_event should succeed");

        match event_back {
            Event::Sample(SampleEvent::PluginStarted) => {}
            other => panic!("expected SampleEvent::PluginStarted, got {:?}", other),
        }
        match payload_back {
            Payload::Sample(ref sp) => assert_eq!(sp.sample_id, 99),
            other => panic!("expected Sample payload, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_notification_fails() {
        let notification = proto::EventNotification { event: None };
        let result = proto_to_event(notification);
        assert!(
            result.is_err(),
            "empty notification should produce an error"
        );
    }
}
