//! The per-job config map sent to plugins (`malbox_plugin_transport::job_config`).

use std::collections::{HashMap, HashSet};

use malbox_database::repositories::artifacts::{self, ArtifactEdge, TaskArtifact};
use malbox_database::repositories::jobs::Job;
use malbox_database::repositories::plugin_reports;
use malbox_database::repositories::tasks::Task;
use malbox_plugin_transport::job_config;
use serde_json::{Value, json};

use crate::OrchestratorCore;
use crate::error::Result;
use crate::params::resolve_plugin_params;
use crate::plan::DetonationRequest;
use crate::routing::AnalyzerSpec;
use crate::submission::task_params;

/// Most indicators handed to one job.
const MAX_INDICATORS: usize = 1000;

/// Earliest incoming edge of `id` that is not a self-loop.
pub(crate) fn earliest_parent_edge(edges: &[ArtifactEdge], id: i64) -> Option<&ArtifactEdge> {
    edges
        .iter()
        .filter(|e| e.child_artifact_id == id && e.parent_artifact_id != id)
        .min_by_key(|e| e.created_on)
}

/// Ancestors of `id` along earliest parent edges, root first, each with the
/// relation of the edge into it (`"root"` for the root).
fn ancestry(edges: &[ArtifactEdge], id: i64) -> Vec<(i64, String)> {
    let mut chain = Vec::new();
    let mut seen = HashSet::from([id]);
    let mut current = id;
    while let Some(edge) = earliest_parent_edge(edges, current) {
        let parent = edge.parent_artifact_id;
        if !seen.insert(parent) {
            break;
        }
        let relation = earliest_parent_edge(edges, parent)
            .map_or_else(|| "root".to_string(), |e| e.relation.clone());
        chain.push((parent, relation));
        current = parent;
    }
    chain.reverse();
    chain
}

/// Display name of an artifact: its earliest incoming edge name, or the
/// task target for the root.
pub(crate) fn artifact_name(
    task: &Task,
    artifact: &TaskArtifact,
    edges: &[ArtifactEdge],
) -> String {
    match earliest_parent_edge(edges, artifact.id) {
        Some(edge) => edge
            .name
            .clone()
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| artifact.sha256.clone()),
        None if !task.target.is_empty() => task.target.clone(),
        None => artifact.sha256.clone(),
    }
}

pub(crate) async fn build_job_config(
    core: &OrchestratorCore,
    task: &Task,
    job: &Job,
    artifact: &TaskArtifact,
    spec: Option<&AnalyzerSpec>,
) -> Result<HashMap<String, String>> {
    let params = task_params(task);
    let all_artifacts = artifacts::fetch_task_artifacts(&core.db, task.id).await?;
    let edges = artifacts::fetch_task_edges(&core.db, task.id).await?;
    let by_id: HashMap<i64, &TaskArtifact> = all_artifacts.iter().map(|a| (a.id, a)).collect();

    let timeout_secs = match spec {
        Some(spec) => spec.timeout.as_secs(),
        None => job
            .detonation
            .clone()
            .and_then(|v| serde_json::from_value::<DetonationRequest>(v).ok())
            .map_or_else(
                || u64::try_from(task.timeout).unwrap_or(0),
                |r| r.timeout_secs,
            ),
    };

    let analysed =
        i64::try_from(all_artifacts.iter().filter(|a| a.analyze).count()).unwrap_or(i64::MAX);
    let remaining = (i64::from(task.max_artifacts) - analysed).max(0);
    let max_artifacts = remaining.min(i64::from(core.settings.max_artifacts_per_job));

    let lineage = ancestry(&edges, artifact.id);
    let ancestry_json: Vec<Value> = lineage
        .iter()
        .filter_map(|(id, relation)| {
            let a = by_id.get(id)?;
            Some(json!({
                "sha256": a.sha256,
                "artifact_type": a.artifact_type,
                "relation": relation,
            }))
        })
        .collect();

    let resolved = spec
        .map(|s| resolve_plugin_params(s, &params))
        .unwrap_or_default();

    let mut config = HashMap::from([
        (
            job_config::ANALYSIS_TIMEOUT.to_string(),
            timeout_secs.to_string(),
        ),
        (job_config::JOB_ID.to_string(), job.id.to_string()),
        (job_config::ARTIFACT_ID.to_string(), artifact.id.to_string()),
        (
            job_config::ARTIFACT_SHA256.to_string(),
            artifact.sha256.clone(),
        ),
        (
            job_config::ARTIFACT_TYPE.to_string(),
            artifact.artifact_type.clone(),
        ),
        (
            job_config::ARTIFACT_NAME.to_string(),
            artifact_name(task, artifact, &edges),
        ),
        (
            job_config::ARTIFACT_DEPTH.to_string(),
            artifact.depth.to_string(),
        ),
        (
            job_config::MAX_ARTIFACTS.to_string(),
            max_artifacts.to_string(),
        ),
        (
            job_config::DEEP_SCAN.to_string(),
            params.deep_scan.to_string(),
        ),
        (job_config::SHARED.to_string(), task.shared_data.to_string()),
        (
            job_config::ANCESTRY.to_string(),
            Value::Array(ancestry_json).to_string(),
        ),
        (
            job_config::PARAMS.to_string(),
            Value::Object(resolved).to_string(),
        ),
    ]);

    if spec.is_some_and(|s| s.uses_indicators) {
        let mut indicators = Vec::new();
        let ids = std::iter::once(artifact.id).chain(lineage.iter().rev().map(|(id, _)| *id));
        for id in ids {
            if indicators.len() >= MAX_INDICATORS {
                break;
            }
            for report in plugin_reports::fetch_plugin_reports_for_artifact(&core.db, id).await? {
                if let Value::Array(items) = report.indicators {
                    indicators.extend(items);
                }
            }
        }
        indicators.truncate(MAX_INDICATORS);
        config.insert(
            job_config::INDICATORS.to_string(),
            Value::Array(indicators).to_string(),
        );
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    fn edge(parent: i64, child: i64, relation: &str, secs: i64) -> ArtifactEdge {
        ArtifactEdge {
            task_id: 1,
            parent_artifact_id: parent,
            child_artifact_id: child,
            relation: relation.into(),
            source_job_id: None,
            source_plugin: None,
            name: Some(format!("n{child}")),
            created_on: OffsetDateTime::UNIX_EPOCH + time::Duration::seconds(secs),
        }
    }

    #[test]
    fn ancestry_follows_earliest_edges_root_first() {
        let edges = vec![
            edge(1, 2, "extracted", 1),
            edge(2, 3, "dropped", 2),
            edge(1, 3, "extracted", 3),
            edge(1, 1, "extracted", 4),
        ];
        assert_eq!(
            ancestry(&edges, 3),
            vec![(1, "root".to_string()), (2, "extracted".to_string())]
        );
        assert!(ancestry(&edges, 1).is_empty());
        assert_eq!(
            earliest_parent_edge(&edges, 3).unwrap().parent_artifact_id,
            2
        );
    }
}
