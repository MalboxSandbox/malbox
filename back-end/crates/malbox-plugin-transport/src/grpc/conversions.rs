//! Proto <-> Event conversions.

use super::proto;
use crate::messages::events::Event;

/// Convert an internal `Event` into a `TaskResult` proto message.
///
/// Used by `GrpcEmitter` to send event metadata back through the
/// `ExecuteTask` streaming response.
pub fn event_to_task_result(task_id: i32, event: &Event) -> proto::TaskResult {
    let result_name = format!("{:?}", event);

    let data = match event {
        Event::TaskCreated { task_id }
        | Event::TaskStarting { task_id }
        | Event::TaskCompleted { task_id }
        | Event::TaskFailed { task_id }
        | Event::TaskCanceled { task_id } => format!(r#"{{"task_id":{}}}"#, task_id).into_bytes(),
        Event::PluginStarted { plugin_id } | Event::PluginStopped { plugin_id } => {
            format!(r#"{{"plugin_id":{}}}"#, plugin_id).into_bytes()
        }
        Event::PluginResultAvailable {
            source,
            result_name,
        } => format!(
            r#"{{"source":"{}","result_name":"{}"}}"#,
            source, result_name
        )
        .into_bytes(),
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

    #[test]
    fn event_to_task_result_task() {
        let event = Event::TaskCreated { task_id: 42 };
        let result = event_to_task_result(100, &event);
        assert_eq!(result.task_id, 100);
        assert!(result.result_name.contains("TaskCreated"));
        assert_eq!(result.data, br#"{"task_id":42}"#);
        assert_eq!(result.format, i32::from(proto::ResultFormat::Json));
        assert!(!result.is_final);
    }

    #[test]
    fn event_to_task_result_plugin() {
        let event = Event::PluginStarted { plugin_id: 5 };
        let result = event_to_task_result(200, &event);
        assert_eq!(result.task_id, 200);
        assert!(result.result_name.contains("PluginStarted"));
        assert_eq!(result.data, br#"{"plugin_id":5}"#);
    }

    #[test]
    fn event_to_task_result_sample() {
        let event = Event::SampleStopped { sample_id: 11 };
        let result = event_to_task_result(300, &event);
        assert_eq!(result.task_id, 300);
        assert!(result.result_name.contains("SampleStopped"));
        assert_eq!(result.data, br#"{"sample_id":11}"#);
    }

    #[test]
    fn event_to_task_result_daemon() {
        let event = Event::DaemonShutdown;
        let result = event_to_task_result(400, &event);
        assert_eq!(result.task_id, 400);
        assert!(result.result_name.contains("DaemonShutdown"));
        assert_eq!(result.data, b"{}");
    }
}
