//! Proto <-> Event conversions.

use super::proto;
use crate::artifact::{ArtifactMeta, ArtifactRelation};
use crate::messages::events::Event;

impl From<ArtifactMeta> for proto::ArtifactMeta {
    fn from(m: ArtifactMeta) -> Self {
        let relation = match m.relation {
            // Plugins never send Root; map it to UNSPECIFIED so the daemon
            // drops the metadata instead of trusting it.
            ArtifactRelation::Root => proto::ArtifactRelation::Unspecified,
            ArtifactRelation::Extracted => proto::ArtifactRelation::Extracted,
            ArtifactRelation::Dropped => proto::ArtifactRelation::Dropped,
            ArtifactRelation::Unpacked => proto::ArtifactRelation::Unpacked,
            ArtifactRelation::Memdump => proto::ArtifactRelation::Memdump,
            ArtifactRelation::Downloaded => proto::ArtifactRelation::Downloaded,
            ArtifactRelation::Pcap => proto::ArtifactRelation::Pcap,
            ArtifactRelation::Output => proto::ArtifactRelation::Output,
        };
        proto::ArtifactMeta {
            relation: relation.into(),
            analyze: m.analyze,
            allow_dynamic_recursion: m.allow_dynamic_recursion,
        }
    }
}

/// Convert proto artifact metadata; `None` for `UNSPECIFIED` or unknown relations.
pub fn artifact_meta_from_proto(m: &proto::ArtifactMeta) -> Option<ArtifactMeta> {
    let relation = u8::try_from(m.relation).ok().filter(|v| *v != 0)?;
    Some(ArtifactMeta {
        relation: ArtifactRelation::try_from(relation).ok()?,
        analyze: m.analyze,
        allow_dynamic_recursion: m.allow_dynamic_recursion,
    })
}

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
        kind: proto::ResultKind::Event.into(),
        artifact: None,
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
        assert_eq!(result.kind, i32::from(proto::ResultKind::Event));
    }

    #[test]
    fn artifact_meta_roundtrip() {
        let meta = ArtifactMeta {
            relation: ArtifactRelation::Unpacked,
            analyze: true,
            allow_dynamic_recursion: true,
        };
        let p = proto::ArtifactMeta::from(meta);
        assert_eq!(artifact_meta_from_proto(&p), Some(meta));
        assert_eq!(
            artifact_meta_from_proto(&proto::ArtifactMeta::default()),
            None
        );
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
