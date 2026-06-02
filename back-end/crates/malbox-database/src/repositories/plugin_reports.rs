use crate::error::{PluginReportError, Result};
use serde::Serialize;
use sqlx::{FromRow, PgPool};
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

/// Row from the `plugin_reports` table.
#[derive(FromRow, Debug, Clone)]
pub struct PluginReport {
    pub id: i32,
    pub task_id: i32,
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

pub async fn insert_plugin_report(
    pool: &PgPool,
    params: &InsertPluginReport<'_>,
) -> Result<PluginReport> {
    sqlx::query_as!(
        PluginReport,
        r#"
        INSERT INTO plugin_reports (
            task_id, plugin_name, display_name, plugin_version,
            classification, score, confidence, labels,
            indicators, ttps, summary, section_count, artifact_count
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        ON CONFLICT (task_id, plugin_name) DO NOTHING
        RETURNING
            id, task_id, plugin_name, display_name, plugin_version,
            classification AS "classification: DbClassification",
            score,
            confidence AS "confidence: DbConfidence",
            labels, indicators, ttps, summary,
            section_count, artifact_count, created_on
        "#,
        params.task_id,
        params.plugin_name,
        params.display_name,
        params.plugin_version,
        params.classification as Option<DbClassification>,
        params.score,
        params.confidence as Option<DbConfidence>,
        params.labels,
        params.indicators,
        params.ttps,
        params.summary,
        params.section_count,
        params.artifact_count,
    )
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

pub async fn fetch_plugin_reports_for_task(
    pool: &PgPool,
    task_id: i32,
) -> Result<Vec<PluginReport>> {
    sqlx::query_as!(
        PluginReport,
        r#"
        SELECT
            id, task_id, plugin_name, display_name, plugin_version,
            classification AS "classification: DbClassification",
            score,
            confidence AS "confidence: DbConfidence",
            labels, indicators, ttps, summary,
            section_count, artifact_count, created_on
        FROM plugin_reports
        WHERE task_id = $1
        ORDER BY plugin_name
        "#,
        task_id,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| PluginReportError::FetchFailed { task_id, source: e }.into())
}

/// Per-plugin latest-wins projection for a sample.
///
/// Returns at most one `PluginReport` per `plugin_name` - the row from the most
/// recent terminal task that ran that plugin. This is the single source of truth
/// for "the sample's current analysis": a re-run enriches (new plugins appear)
/// and overrides (a re-run plugin replaces its prior row) at once, while every
/// task's rows remain in the table as history.
pub async fn fetch_latest_plugin_reports_for_sample(
    pool: &PgPool,
    sample_id: i64,
) -> Result<Vec<PluginReport>> {
    sqlx::query_as!(
        PluginReport,
        r#"
        SELECT DISTINCT ON (pr.plugin_name)
            pr.id, pr.task_id, pr.plugin_name, pr.display_name, pr.plugin_version,
            pr.classification AS "classification: DbClassification",
            pr.score,
            pr.confidence AS "confidence: DbConfidence",
            pr.labels, pr.indicators, pr.ttps, pr.summary,
            pr.section_count, pr.artifact_count, pr.created_on
        FROM plugin_reports pr
        JOIN tasks t ON t.id = pr.task_id
        WHERE t.sample_id = $1
          AND t.status IN ('completed', 'failed', 'canceled')
        ORDER BY pr.plugin_name, t.created_on DESC, pr.task_id DESC
        "#,
        sample_id,
    )
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

/// Count distinct terminal tasks that produced at least one plugin report for
/// this sample. Used for the `task_count` headline (the latest-wins projection
/// collapses to one row per plugin, so counting its task_ids would undercount
/// the number of runs).
pub async fn count_reported_tasks_for_sample(pool: &PgPool, sample_id: i64) -> Result<i64> {
    let row = sqlx::query!(
        r#"
        SELECT COUNT(DISTINCT pr.task_id) AS "count!"
        FROM plugin_reports pr
        JOIN tasks t ON t.id = pr.task_id
        WHERE t.sample_id = $1
          AND t.status IN ('completed', 'failed', 'canceled')
        "#,
        sample_id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| PluginReportError::FetchForSampleFailed {
        sample_id,
        source: e,
    })?;
    Ok(row.count)
}
