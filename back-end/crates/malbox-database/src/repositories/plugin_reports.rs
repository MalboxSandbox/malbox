use crate::error::{PluginReportError, Result};
use serde::Serialize;
use sqlx::{AssertSqlSafe, FromRow, PgConnection, PgPool};
use time::OffsetDateTime;

/// Mirrors the `classification` Postgres enum. Ordered by severity so that
/// `max(classification)` in SQL yields worst-wins aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize)]
#[sqlx(type_name = "classification", rename_all = "lowercase")]
pub enum DbClassification {
    Clean,
    Unknown,
    Suspicious,
    Malicious,
}

impl DbClassification {
    pub fn severity(self) -> u8 {
        match self {
            Self::Clean => 0,
            Self::Unknown => 1,
            Self::Suspicious => 2,
            Self::Malicious => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Unknown => "unknown",
            Self::Suspicious => "suspicious",
            Self::Malicious => "malicious",
        }
    }
}

/// Mirrors the `confidence` Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize)]
#[sqlx(type_name = "confidence", rename_all = "lowercase")]
pub enum DbConfidence {
    Low,
    Medium,
    High,
}

impl DbConfidence {
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

/// Every column of `plugin_reports`, unqualified.
const REPORT_COLUMNS: &str = "id, task_id, artifact_id, job_id, report_path, plugin_name, \
    display_name, plugin_version, classification, score, confidence, labels, indicators, ttps, \
    summary, section_count, artifact_count, created_on";

/// Every column of `plugin_reports`, qualified with the `pr` alias.
const PR_COLUMNS: &str = "pr.id, pr.task_id, pr.artifact_id, pr.job_id, pr.report_path, \
    pr.plugin_name, pr.display_name, pr.plugin_version, pr.classification, pr.score, \
    pr.confidence, pr.labels, pr.indicators, pr.ttps, pr.summary, pr.section_count, \
    pr.artifact_count, pr.created_on";

/// Non-key columns overwritten on upsert conflicts.
const UPSERT_SET: &str = "task_id = excluded.task_id, job_id = excluded.job_id, \
    report_path = excluded.report_path, display_name = excluded.display_name, \
    plugin_version = excluded.plugin_version, classification = excluded.classification, \
    score = excluded.score, confidence = excluded.confidence, labels = excluded.labels, \
    indicators = excluded.indicators, ttps = excluded.ttps, summary = excluded.summary, \
    section_count = excluded.section_count, artifact_count = excluded.artifact_count, \
    created_on = now()";

/// Row from the `plugin_reports` table.
#[derive(FromRow, Debug, Clone)]
pub struct PluginReport {
    pub id: i32,
    pub task_id: i32,
    pub artifact_id: i64,
    pub job_id: Option<i64>,
    /// Report JSON path, relative to the data directory.
    pub report_path: String,
    pub plugin_name: String,
    pub display_name: Option<String>,
    pub plugin_version: String,
    pub classification: Option<DbClassification>,
    pub score: Option<i16>,
    pub confidence: Option<DbConfidence>,
    pub labels: Vec<String>,
    pub indicators: serde_json::Value,
    pub ttps: serde_json::Value,
    pub summary: Option<String>,
    pub section_count: i32,
    pub artifact_count: i32,
    pub created_on: OffsetDateTime,
}

pub struct InsertPluginReport<'a> {
    pub task_id: i32,
    pub artifact_id: i64,
    pub job_id: Option<i64>,
    pub report_path: &'a str,
    pub plugin_name: &'a str,
    pub display_name: Option<&'a str>,
    pub plugin_version: &'a str,
    pub classification: Option<DbClassification>,
    pub score: Option<i16>,
    pub confidence: Option<DbConfidence>,
    pub labels: &'a [String],
    pub indicators: &'a serde_json::Value,
    pub ttps: &'a serde_json::Value,
    pub summary: Option<&'a str>,
    pub section_count: i32,
    pub artifact_count: i32,
}

/// Insert a plugin report, replacing an existing row for the same
/// `(artifact_id, plugin_name)`.
pub async fn upsert_plugin_report(
    pool: &PgPool,
    params: &InsertPluginReport<'_>,
) -> Result<PluginReport> {
    let sql = format!(
        "insert into plugin_reports (task_id, artifact_id, job_id, report_path, plugin_name, \
         display_name, plugin_version, classification, score, confidence, labels, indicators, \
         ttps, summary, section_count, artifact_count) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16) \
         on conflict (artifact_id, plugin_name) do update set {UPSERT_SET} \
         returning {REPORT_COLUMNS}"
    );
    sqlx::query_as::<_, PluginReport>(AssertSqlSafe(sql))
        .bind(params.task_id)
        .bind(params.artifact_id)
        .bind(params.job_id)
        .bind(params.report_path)
        .bind(params.plugin_name)
        .bind(params.display_name)
        .bind(params.plugin_version)
        .bind(params.classification)
        .bind(params.score)
        .bind(params.confidence)
        .bind(params.labels)
        .bind(params.indicators)
        .bind(params.ttps)
        .bind(params.summary)
        .bind(params.section_count)
        .bind(params.artifact_count)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            PluginReportError::InsertFailed {
                task_id: params.task_id,
                plugin_name: params.plugin_name.to_string(),
                source: e,
            }
            .into()
        })
}

/// Copy the report produced by `from_job_id` onto another artifact/job/task
/// (cache replay). Returns `None` when the source job produced no report.
pub async fn copy_plugin_report(
    conn: &mut PgConnection,
    from_job_id: i64,
    to_artifact_id: i64,
    to_job_id: i64,
    to_task_id: i32,
) -> Result<Option<PluginReport>> {
    let sql = format!(
        "insert into plugin_reports (task_id, artifact_id, job_id, report_path, plugin_name, \
         display_name, plugin_version, classification, score, confidence, labels, indicators, \
         ttps, summary, section_count, artifact_count) \
         select $4, $2, $3, report_path, plugin_name, display_name, plugin_version, \
         classification, score, confidence, labels, indicators, ttps, summary, section_count, \
         artifact_count from plugin_reports where job_id = $1 \
         on conflict (artifact_id, plugin_name) do update set {UPSERT_SET} \
         returning {REPORT_COLUMNS}"
    );
    sqlx::query_as::<_, PluginReport>(AssertSqlSafe(sql))
        .bind(from_job_id)
        .bind(to_artifact_id)
        .bind(to_job_id)
        .bind(to_task_id)
        .fetch_optional(conn)
        .await
        .map_err(|source| {
            PluginReportError::CopyFailed {
                from_job_id,
                to_task_id,
                source,
            }
            .into()
        })
}

pub async fn fetch_plugin_reports_for_task(
    pool: &PgPool,
    task_id: i32,
) -> Result<Vec<PluginReport>> {
    let sql = format!(
        "select {REPORT_COLUMNS} from plugin_reports where task_id = $1 \
         order by artifact_id, plugin_name"
    );
    sqlx::query_as::<_, PluginReport>(AssertSqlSafe(sql))
        .bind(task_id)
        .fetch_all(pool)
        .await
        .map_err(|e| PluginReportError::FetchFailed { task_id, source: e }.into())
}

pub async fn fetch_plugin_reports_for_artifact(
    pool: &PgPool,
    artifact_id: i64,
) -> Result<Vec<PluginReport>> {
    let sql = format!(
        "select {REPORT_COLUMNS} from plugin_reports where artifact_id = $1 order by plugin_name"
    );
    sqlx::query_as::<_, PluginReport>(AssertSqlSafe(sql))
        .bind(artifact_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            PluginReportError::FetchForArtifactFailed {
                artifact_id,
                source: e,
            }
            .into()
        })
}

pub async fn fetch_plugin_report(
    pool: &PgPool,
    artifact_id: i64,
    plugin_name: &str,
) -> Result<Option<PluginReport>> {
    let sql = format!(
        "select {REPORT_COLUMNS} from plugin_reports where artifact_id = $1 and plugin_name = $2"
    );
    sqlx::query_as::<_, PluginReport>(AssertSqlSafe(sql))
        .bind(artifact_id)
        .bind(plugin_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            PluginReportError::FetchForArtifactFailed {
                artifact_id,
                source: e,
            }
            .into()
        })
}

/// Per-plugin latest-wins projection for a sample.
///
/// Returns at most one `PluginReport` per `plugin_name` - the row from the most
/// recent terminal task in which an artifact with this sample ran that plugin.
/// This is the single source of truth for "the sample's current analysis": a
/// re-run enriches (new plugins appear) and overrides (a re-run plugin replaces
/// its prior row) at once, while every task's rows remain in the table as history.
pub async fn fetch_latest_plugin_reports_for_sample(
    pool: &PgPool,
    sample_id: i64,
) -> Result<Vec<PluginReport>> {
    let sql = format!(
        "select distinct on (pr.plugin_name) {PR_COLUMNS} \
         from plugin_reports pr \
         join task_artifacts ta on ta.id = pr.artifact_id \
         join tasks t on t.id = ta.task_id \
         where ta.sample_id = $1 and t.status in ('completed', 'failed', 'canceled') \
         order by pr.plugin_name, t.created_on desc, pr.task_id desc"
    );
    sqlx::query_as::<_, PluginReport>(AssertSqlSafe(sql))
        .bind(sample_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            PluginReportError::FetchForSampleFailed {
                sample_id,
                source: e,
            }
            .into()
        })
}

/// Latest report of one plugin for a sample, over every task (running ones
/// included) in which an artifact with this sample was analysed. Returns the
/// report and the task it belongs to.
pub async fn fetch_latest_plugin_report_for_sample(
    pool: &PgPool,
    sample_id: i64,
    plugin_name: &str,
) -> Result<Option<(PluginReport, i32)>> {
    let sql = format!(
        "select {PR_COLUMNS} from plugin_reports pr \
         join task_artifacts ta on ta.id = pr.artifact_id \
         join tasks t on t.id = ta.task_id \
         where ta.sample_id = $1 and pr.plugin_name = $2 \
         order by t.created_on desc, pr.task_id desc, pr.id desc limit 1"
    );
    let report = sqlx::query_as::<_, PluginReport>(AssertSqlSafe(sql))
        .bind(sample_id)
        .bind(plugin_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| PluginReportError::FetchForSampleFailed {
            sample_id,
            source: e,
        })?;
    Ok(report.map(|r| {
        let task_id = r.task_id;
        (r, task_id)
    }))
}

/// Count distinct terminal tasks that produced at least one plugin report for
/// this sample. Used for the `task_count` headline (the latest-wins projection
/// collapses to one row per plugin, so counting its task_ids would undercount
/// the number of runs).
pub async fn count_reported_tasks_for_sample(pool: &PgPool, sample_id: i64) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "select count(distinct pr.task_id) from plugin_reports pr \
         join task_artifacts ta on ta.id = pr.artifact_id \
         join tasks t on t.id = ta.task_id \
         where ta.sample_id = $1 and t.status in ('completed', 'failed', 'canceled')",
    )
    .bind(sample_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        PluginReportError::FetchForSampleFailed {
            sample_id,
            source: e,
        }
        .into()
    })
}
