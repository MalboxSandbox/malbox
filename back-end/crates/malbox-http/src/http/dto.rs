//! Shared response DTOs for the task and sample surfaces.
//!
//! These replace the previously-duplicated `SampleInfo` (x3), `TaskResponse`
//! (x2), and `SinglePluginReportResponse` (x2) structs. Task and sample reports
//! serialize the same `ReportBody` (flattened) under different subject keys.

use malbox_database::PgPool;
use malbox_database::repositories::samples::{SampleEntity, fetch_sample_by_id};
use malbox_database::repositories::tasks::Task;
use malbox_plugin_sdk::report::{Indicator, Ttp};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::http::report_service::ArtifactLink;

/// File identity for a sample. Includes `id` (the previous task-side
/// `SampleInfo` omitted it; unifying on the superset is harmless to the FE).
/// `Clone` so the task-list batch loader can attach one sample to several tasks.
#[derive(Serialize, Clone)]
pub struct SampleDto {
    pub id: i64,
    pub file_size: i64,
    pub file_type: String,
    pub md5: String,
    pub crc32: String,
    pub sha1: String,
    pub sha256: String,
    pub sha512: String,
    pub ssdeep: String,
}

impl From<SampleEntity> for SampleDto {
    fn from(s: SampleEntity) -> Self {
        SampleDto {
            id: s.id,
            file_size: s.file_size,
            file_type: s.file_type,
            md5: s.md5,
            crc32: s.crc32,
            sha1: s.sha1,
            sha256: s.sha256,
            sha512: s.sha512,
            ssdeep: s.ssdeep,
        }
    }
}

/// A task as returned by the read endpoints. `sample` is attached by the
/// handler (batch-loaded), not by `From<Task>`.
#[derive(Serialize)]
pub struct TaskDto {
    pub id: i32,
    pub status: String,
    pub target: String,
    pub platform: String,
    pub timeout: i64,
    pub priority: i64,
    pub owner: Option<String>,
    pub machine_id: Option<i32>,
    pub plugins: Vec<String>,
    pub tags: Option<Vec<String>>,
    pub created_on: String,
    pub started_on: Option<String>,
    pub completed_on: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample: Option<SampleDto>,
}

impl From<Task> for TaskDto {
    fn from(t: Task) -> Self {
        TaskDto {
            id: t.id.unwrap_or_default(),
            status: crate::http::report_service::task_status_str(&t.status),
            target: t.target,
            platform: t
                .platform
                .map(|p| format!("{:?}", p).to_lowercase())
                .unwrap_or_default(),
            timeout: t.timeout,
            priority: t.priority,
            owner: t.owner,
            machine_id: t.machine_id,
            plugins: t.plugins,
            tags: t.tags,
            created_on: t.created_on.to_string(),
            started_on: t.started_on.map(|d| d.to_string()),
            completed_on: t.completed_on.map(|d| d.to_string()),
            sample: None,
        }
    }
}

/// Cross-plugin rollup. Identical wire shape to the FE `AggregateView`.
#[derive(Serialize, Default)]
pub struct Aggregate {
    pub verdict: Option<String>,
    pub score: Option<u8>,
    pub classifications: BTreeMap<String, u32>,
    pub indicators: Vec<Indicator>,
    pub ttps: Vec<Ttp>,
    pub plugin_count: u32,
    pub report_count: u32,
}

/// One plugin's view within a report. `source_task_id` is populated only on the
/// sample rollup (which task produced this plugin's current result); omitted on
/// the per-task report.
#[derive(Serialize)]
pub struct PluginReportView {
    pub plugin_name: String,
    pub report: Option<serde_json::Value>,
    pub synthesized: bool,
    pub failed: bool,
    pub artifacts: Vec<ArtifactLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_task_id: Option<i32>,
}

/// The shared report body, flattened under a subject key by the wrappers below.
#[derive(Serialize)]
pub struct ReportBody {
    pub aggregate: Aggregate,
    pub plugins: Vec<PluginReportView>,
}

#[derive(Serialize)]
pub struct TaskReportResponse {
    pub task: TaskDto,
    #[serde(flatten)]
    pub body: ReportBody,
}

#[derive(Serialize)]
pub struct SampleReportResponse {
    pub sample: SampleDto,
    #[serde(flatten)]
    pub body: ReportBody,
}

/// Single-plugin full report (with sections). One definition for both surfaces;
/// `task_id` is always present (on the task surface it equals the path id; on
/// the sample surface it is the latest task that ran the plugin).
#[derive(Serialize)]
pub struct SinglePluginReportResponse {
    pub plugin_name: String,
    pub task_id: i32,
    pub report: Option<serde_json::Value>,
    pub synthesized: bool,
    pub failed: bool,
    pub artifacts: Vec<ArtifactLink>,
}

/// Build a TaskDto for a single task, attaching its sample (one extra query).
pub async fn attach_one_task(pool: &PgPool, task: Task) -> TaskDto {
    let sample_id = task.sample_id;
    let mut dto = TaskDto::from(task);
    if let Some(sid) = sample_id
        && let Ok(Some(s)) = fetch_sample_by_id(pool, sid).await
    {
        dto.sample = Some(SampleDto::from(s));
    }
    dto
}
