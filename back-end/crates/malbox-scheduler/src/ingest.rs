//! The only writer of job outputs: reports, shared data, drop requests and
//! artifacts.

use malbox_database::repositories::artifacts::{self, NewArtifact, TaskArtifact};
use malbox_database::repositories::jobs::{self, Job};
use malbox_database::repositories::plugin_reports::{
    self, DbClassification, DbConfidence, InsertPluginReport,
};
use malbox_database::repositories::tasks::Task;
use malbox_plugin_internal::manager::handle::{OutputFormat, PluginOutput};
use malbox_plugin_sdk::report::{Classification, Confidence, Report};
use malbox_plugin_transport::artifact::{ArtifactMeta, ArtifactRelation};
use malbox_plugin_transport::{DROP_RESULT_NAME, REPORT_RESULT_NAME, SHARED_RESULT_NAME};
use malbox_utils::hashing;
use serde_json::{Map, Value};
use tracing::warn;

use crate::OrchestratorCore;
use crate::error::{Result, SchedulerError};
use crate::submission::relative_storage_path;

pub struct IngestContext {
    pub task: Task,
    pub job: Job,
    /// The job's input artifact.
    pub artifact: TaskArtifact,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct IngestSummary {
    /// Artifacts created, or whose analysis was newly requested.
    pub new_artifacts: Vec<i64>,
    pub had_report: bool,
    pub changed_shared_keys: Vec<String>,
    pub drop_requested: bool,
    /// Artifacts discarded by `max_artifacts_per_job`.
    pub discarded: u32,
    pub invalid_report: Option<String>,
    /// Some artifact was kept without its blob because the task's
    /// `max_total_bytes` was spent: the outputs depend on this task's budget.
    pub budget_limited: bool,
}

impl IngestSummary {
    /// Fold another plugin's summary (detonations ingest per plugin).
    pub fn merge(&mut self, other: IngestSummary) {
        for id in other.new_artifacts {
            if !self.new_artifacts.contains(&id) {
                self.new_artifacts.push(id);
            }
        }
        self.had_report |= other.had_report;
        for k in other.changed_shared_keys {
            if !self.changed_shared_keys.contains(&k) {
                self.changed_shared_keys.push(k);
            }
        }
        self.drop_requested |= other.drop_requested;
        self.discarded += other.discarded;
        self.budget_limited |= other.budget_limited;
        if self.invalid_report.is_none() {
            self.invalid_report = other.invalid_report;
        }
    }

    /// The job message for discarded artifacts, if any.
    pub fn discard_message(&self) -> Option<String> {
        (self.discarded > 0).then(|| {
            format!(
                "{} artifacts discarded: max_artifacts_per_job reached",
                self.discarded
            )
        })
    }
}

fn sdk_classification(c: Classification) -> DbClassification {
    match c {
        Classification::Clean => DbClassification::Clean,
        Classification::Unknown => DbClassification::Unknown,
        Classification::Suspicious => DbClassification::Suspicious,
        Classification::Malicious => DbClassification::Malicious,
    }
}

fn sdk_confidence(c: Confidence) -> DbConfidence {
    match c {
        Confidence::Low => DbConfidence::Low,
        Confidence::Medium => DbConfidence::Medium,
        Confidence::High => DbConfidence::High,
    }
}

/// Longest edge name stored; longer names are cut at a char boundary.
const MAX_EDGE_NAME_CHARS: usize = 1024;

/// Control results (`report`, `malbox.shared`, `malbox.drop`) are JSON
/// without artifact metadata. Names are not enough: they can come from the
/// sample (an archive entry or dropped file called `malbox.drop`), and those
/// outputs are artifacts.
fn is_control(o: &PluginOutput, name: &str) -> bool {
    o.artifact.is_none() && o.format == OutputFormat::Json && o.result_name == name
}

fn is_report(o: &PluginOutput) -> bool {
    is_control(o, REPORT_RESULT_NAME)
}

fn is_artifact(o: &PluginOutput) -> bool {
    !is_report(o) && !is_control(o, SHARED_RESULT_NAME) && !is_control(o, DROP_RESULT_NAME)
}

/// Edge name as stored: Postgres text rejects NUL, and names derived from
/// sample paths are unbounded.
fn edge_name(name: &str) -> String {
    name.chars()
        .filter(|c| *c != '\0')
        .take(MAX_EDGE_NAME_CHARS)
        .collect()
}

/// Merge `update` into `current`: arrays union (insertion order kept),
/// everything else overwrites. Returns the keys whose value changed.
pub fn merge_shared(current: &mut Map<String, Value>, update: Map<String, Value>) -> Vec<String> {
    let mut changed = Vec::new();
    for (key, value) in update {
        let merged = match (current.get(&key), value) {
            (Some(Value::Array(old)), Value::Array(new)) => {
                let mut out = old.clone();
                for v in new {
                    if !out.contains(&v) {
                        out.push(v);
                    }
                }
                Value::Array(out)
            }
            (_, v) => v,
        };
        if current.get(&key) != Some(&merged) {
            changed.push(key.clone());
            current.insert(key, merged);
        }
    }
    changed
}

pub async fn ingest_outputs(
    core: &OrchestratorCore,
    ctx: &IngestContext,
    plugin_name: &str,
    outputs: Vec<PluginOutput>,
) -> Result<IngestSummary> {
    let mut summary = IngestSummary::default();
    let per_job_cap = core.settings.max_artifacts_per_job as usize;
    let artifact_count = outputs
        .iter()
        .filter(|o| is_artifact(o))
        .count()
        .min(per_job_cap);
    let mut ingested = 0usize;

    for output in outputs {
        if is_report(&output) {
            ingest_report(
                core,
                ctx,
                plugin_name,
                &output.data,
                artifact_count,
                &mut summary,
            )
            .await?;
        } else if is_control(&output, SHARED_RESULT_NAME) {
            ingest_shared(core, ctx, &output.data, &mut summary).await?;
        } else if is_control(&output, DROP_RESULT_NAME) {
            jobs::set_drop_requested(&core.db, ctx.job.id).await?;
            summary.drop_requested = true;
        } else if ingested >= per_job_cap {
            summary.discarded += 1;
        } else {
            ingested += 1;
            ingest_artifact(core, ctx, plugin_name, output, &mut summary).await?;
        }
    }
    Ok(summary)
}

async fn ingest_report(
    core: &OrchestratorCore,
    ctx: &IngestContext,
    plugin_name: &str,
    data: &[u8],
    artifact_count: usize,
    summary: &mut IngestSummary,
) -> Result<()> {
    let report = match serde_json::from_slice::<Report>(data) {
        Ok(r) => r,
        Err(e) => {
            warn!(task_id = ctx.task.id, plugin_name, error = %e, "invalid report");
            summary.invalid_report = Some(e.to_string());
            return Ok(());
        }
    };
    let report_path = core
        .result_store
        .store_report(ctx.task.id, ctx.artifact.id, plugin_name, data)
        .await?;

    let labels: Vec<String> = report
        .verdict
        .as_ref()
        .map(|v| v.labels.clone())
        .unwrap_or_default();
    let indicators = serde_json::to_value(&report.indicators).unwrap_or_default();
    let ttps = serde_json::to_value(&report.ttps).unwrap_or_default();
    plugin_reports::upsert_plugin_report(
        &core.db,
        &InsertPluginReport {
            task_id: ctx.task.id,
            artifact_id: ctx.artifact.id,
            job_id: Some(ctx.job.id),
            report_path: &report_path,
            plugin_name,
            display_name: report.plugin.display_name.as_deref(),
            plugin_version: &report.plugin.version,
            classification: report
                .verdict
                .as_ref()
                .map(|v| sdk_classification(v.classification)),
            score: report.verdict.as_ref().and_then(|v| v.score.map(i16::from)),
            confidence: report
                .verdict
                .as_ref()
                .and_then(|v| v.confidence.map(sdk_confidence)),
            labels: &labels,
            indicators: &indicators,
            ttps: &ttps,
            summary: report.summary.as_deref(),
            section_count: report.sections.len() as i32,
            artifact_count: artifact_count as i32,
        },
    )
    .await?;
    summary.had_report = true;
    Ok(())
}

async fn ingest_shared(
    core: &OrchestratorCore,
    ctx: &IngestContext,
    data: &[u8],
    summary: &mut IngestSummary,
) -> Result<()> {
    let update = match serde_json::from_slice::<Value>(data) {
        Ok(Value::Object(m)) => m,
        Ok(_) | Err(_) => {
            warn!(
                task_id = ctx.task.id,
                job_id = ctx.job.id,
                "malbox.shared is not a JSON object, ignored"
            );
            return Ok(());
        }
    };
    let mut tx = core.db.begin().await?;
    let mut current =
        match artifacts::fetch_task_shared_data_for_update(&mut *tx, ctx.task.id).await? {
            Value::Object(m) => m,
            _ => Map::new(),
        };
    let changed = merge_shared(&mut current, update);
    if !changed.is_empty() {
        artifacts::set_task_shared_data(&mut *tx, ctx.task.id, &Value::Object(current)).await?;
    }
    tx.commit().await?;
    for k in changed {
        if !summary.changed_shared_keys.contains(&k) {
            summary.changed_shared_keys.push(k);
        }
    }
    Ok(())
}

async fn ingest_artifact(
    core: &OrchestratorCore,
    ctx: &IngestContext,
    plugin_name: &str,
    output: PluginOutput,
    summary: &mut IngestSummary,
) -> Result<()> {
    let meta = output.artifact.unwrap_or(ArtifactMeta {
        relation: ArtifactRelation::Output,
        analyze: false,
        allow_dynamic_recursion: false,
    });
    let name = edge_name(&output.result_name);
    let data = output.data;
    let size = data.len() as i64;

    let identify = core.identify;
    let id_name = name.clone();
    let (data, sha256, ident) = tokio::task::spawn_blocking(move || {
        let sha256 = hashing::get_sha256(&data);
        let ident = identify(&data, Some(&id_name));
        (data, sha256, ident)
    })
    .await
    .map_err(|e| SchedulerError::Internal(format!("identification task failed: {e}")))?;

    // The task row stays locked until commit, so concurrent jobs of this
    // task check `max_total_bytes` one at a time.
    let mut tx = core.db.begin().await?;
    let used = artifacts::lock_task_artifact_bytes(&mut tx, ctx.task.id).await?;
    let over_budget = used.saturating_add(size) > ctx.task.max_total_bytes;
    let storage_path = if over_budget {
        None
    } else {
        let path = core.sample_store.store(&sha256, &data).await?;
        Some(relative_storage_path(&core.data_dir, &path))
    };
    let (state, drop_reason) = if over_budget {
        summary.budget_limited = true;
        ("dropped", Some("max_total_bytes"))
    } else {
        ("stored", None)
    };

    let conn = &mut *tx;
    let (child, created) = artifacts::insert_or_get_artifact(
        &mut *conn,
        &NewArtifact {
            task_id: ctx.task.id,
            sha256: &sha256,
            size_bytes: size,
            storage_path: storage_path.as_deref(),
            analyze: false,
            sample_id: None,
            depth: ctx.artifact.depth + 1,
            artifact_type: &ident.artifact_type,
            mime: &ident.mime,
            state,
            drop_reason,
            priority: ctx.artifact.priority,
            analyze_requested: meta.analyze,
            dynamic_recursion_bypass: meta.allow_dynamic_recursion,
        },
    )
    .await?;
    let newly_requested = !created && meta.analyze && !child.analyze_requested;
    if !created && meta.analyze {
        artifacts::set_analyze_requested(&mut *conn, child.id, meta.allow_dynamic_recursion)
            .await?;
    }
    artifacts::insert_edge(
        &mut *conn,
        ctx.task.id,
        ctx.artifact.id,
        child.id,
        meta.relation.as_str(),
        Some(ctx.job.id),
        Some(plugin_name),
        Some(&name),
    )
    .await?;
    tx.commit().await?;

    if (created || newly_requested) && !summary.new_artifacts.contains(&child.id) {
        summary.new_artifacts.push(child.id);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn shared_arrays_union_in_order() {
        let mut cur = obj(json!({"passwords": ["a", "b"]}));
        let changed = merge_shared(&mut cur, obj(json!({"passwords": ["b", "c"]})));
        assert_eq!(changed, vec!["passwords"]);
        assert_eq!(cur["passwords"], json!(["a", "b", "c"]));
    }

    #[test]
    fn shared_unchanged_value_not_reported() {
        let mut cur = obj(json!({"passwords": ["x"], "k": 1}));
        let changed = merge_shared(&mut cur, obj(json!({"passwords": ["x"], "k": 1})));
        assert!(changed.is_empty());
    }

    #[test]
    fn shared_scalars_overwrite() {
        let mut cur = obj(json!({"k": 1, "a": [1]}));
        let changed = merge_shared(&mut cur, obj(json!({"k": 2, "a": "s", "new": true})));
        assert_eq!(changed, vec!["a", "k", "new"]);
        assert_eq!(cur, obj(json!({"k": 2, "a": "s", "new": true})));
    }

    #[test]
    fn summary_merge() {
        let mut a = IngestSummary {
            new_artifacts: vec![1],
            discarded: 1,
            ..Default::default()
        };
        a.merge(IngestSummary {
            new_artifacts: vec![1, 2],
            had_report: true,
            changed_shared_keys: vec!["k".into()],
            drop_requested: true,
            discarded: 2,
            invalid_report: Some("bad".into()),
            budget_limited: true,
        });
        assert_eq!(a.new_artifacts, vec![1, 2]);
        assert!(a.had_report && a.drop_requested && a.budget_limited);
        assert_eq!(a.discarded, 3);
        assert_eq!(
            a.discard_message().as_deref(),
            Some("3 artifacts discarded: max_artifacts_per_job reached")
        );
        assert_eq!(a.invalid_report.as_deref(), Some("bad"));
    }

    fn output(name: &str, format: OutputFormat, artifact: Option<ArtifactMeta>) -> PluginOutput {
        PluginOutput {
            result_name: name.into(),
            data: b"{}".to_vec(),
            format,
            artifact,
        }
    }

    #[test]
    fn reserved_names_from_samples_are_artifacts() {
        let meta = ArtifactMeta {
            relation: ArtifactRelation::Extracted,
            analyze: true,
            allow_dynamic_recursion: false,
        };
        for name in [DROP_RESULT_NAME, SHARED_RESULT_NAME, REPORT_RESULT_NAME] {
            // Archive entry pushed via push_artifact.
            assert!(is_artifact(&output(name, OutputFormat::Json, Some(meta))));
            // Auto-collected file without a rule: a plain byte result.
            assert!(is_artifact(&output(name, OutputFormat::Bytes, None)));
            // The SDK's own control result.
            assert!(!is_artifact(&output(name, OutputFormat::Json, None)));
        }
    }

    #[test]
    fn edge_name_strips_nul_and_caps_length() {
        assert_eq!(edge_name("a\0b"), "ab");
        let long = "é".repeat(MAX_EDGE_NAME_CHARS + 10);
        assert_eq!(edge_name(&long).chars().count(), MAX_EDGE_NAME_CHARS);
    }
}
