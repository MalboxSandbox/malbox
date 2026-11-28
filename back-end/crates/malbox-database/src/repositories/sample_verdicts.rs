//! `sample_verdicts`: per-sample verdict aggregated over all of its plugin reports.

use crate::error::{Result, SampleVerdictError};
use crate::repositories::plugin_reports::DbClassification;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;

/// Row from the `sample_verdicts` table.
#[derive(FromRow, Debug, Clone)]
pub struct SampleVerdict {
    /// Sample the verdict belongs to.
    pub sample_id: i64,
    /// Worst classification across the sample's reports.
    pub classification: Option<DbClassification>,
    /// Aggregated threat score.
    pub score: Option<i16>,
    /// Total indicators across the sample's reports.
    pub indicator_count: i32,
    /// Total attacker techniques across the sample's reports.
    pub ttp_count: i32,
    /// Plugins that contributed a report.
    pub plugin_names: Vec<String>,
    /// Number of tasks that analysed the sample.
    pub task_count: i32,
    /// Most recent task that analysed the sample.
    pub last_task_id: Option<i32>,
    /// When the verdict was last recomputed.
    pub updated_on: OffsetDateTime,
}

/// Column values for [`upsert_sample_verdict`]; see [`SampleVerdict`] for meanings.
#[derive(Debug)]
pub struct UpsertSampleVerdict<'a> {
    /// Sample the verdict belongs to.
    pub sample_id: i64,
    /// Worst classification across the sample's reports.
    pub classification: Option<DbClassification>,
    /// Aggregated threat score.
    pub score: Option<i16>,
    /// Total indicators across the sample's reports.
    pub indicator_count: i32,
    /// Total attacker techniques across the sample's reports.
    pub ttp_count: i32,
    /// Plugins that contributed a report.
    pub plugin_names: &'a [String],
    /// Number of tasks that analysed the sample.
    pub task_count: i32,
    /// Most recent task that analysed the sample.
    pub last_task_id: Option<i32>,
}

/// Insert or replace the verdict row of a sample.
///
/// # Errors
///
/// Returns [`SampleVerdictError::UpsertFailed`] when the upsert fails.
#[expect(
    clippy::as_conversions,
    reason = "`as Option<DbClassification>` is sqlx `query!` type-override syntax, not a cast"
)]
pub async fn upsert_sample_verdict(pool: &PgPool, params: &UpsertSampleVerdict<'_>) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO sample_verdicts (
            sample_id, classification, score,
            indicator_count, ttp_count, plugin_names,
            task_count, last_task_id, updated_on
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, now())
        ON CONFLICT (sample_id) DO UPDATE SET
            classification  = EXCLUDED.classification,
            score           = EXCLUDED.score,
            indicator_count = EXCLUDED.indicator_count,
            ttp_count       = EXCLUDED.ttp_count,
            plugin_names    = EXCLUDED.plugin_names,
            task_count      = EXCLUDED.task_count,
            last_task_id    = EXCLUDED.last_task_id,
            updated_on      = EXCLUDED.updated_on
        "#,
        params.sample_id,
        params.classification as Option<DbClassification>,
        params.score,
        params.indicator_count,
        params.ttp_count,
        params.plugin_names,
        params.task_count,
        params.last_task_id,
    )
    .execute(pool)
    .await
    .map_err(|e| SampleVerdictError::UpsertFailed {
        sample_id: params.sample_id,
        source: e,
    })?;
    Ok(())
}

/// Fetch the verdict of a sample by id, `None` when none was computed yet.
///
/// # Errors
///
/// Returns [`SampleVerdictError::FetchFailed`] when the query fails.
pub async fn fetch_sample_verdict(pool: &PgPool, sample_id: i64) -> Result<Option<SampleVerdict>> {
    sqlx::query_as!(
        SampleVerdict,
        r#"
        SELECT
            sample_id, classification AS "classification: DbClassification",
            score, indicator_count, ttp_count, plugin_names,
            task_count, last_task_id, updated_on
        FROM sample_verdicts
        WHERE sample_id = $1
        "#,
        sample_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SampleVerdictError::FetchFailed { source: e }.into())
}

/// Fetch the verdict of the sample with the given SHA-256, `None` when absent.
///
/// # Errors
///
/// Returns [`SampleVerdictError::FetchFailed`] when the query fails.
pub async fn fetch_sample_verdict_by_sha256(
    pool: &PgPool,
    sha256: &str,
) -> Result<Option<SampleVerdict>> {
    sqlx::query_as!(
        SampleVerdict,
        r#"
        SELECT
            sv.sample_id, sv.classification AS "classification: DbClassification",
            sv.score, sv.indicator_count, sv.ttp_count, sv.plugin_names,
            sv.task_count, sv.last_task_id, sv.updated_on
        FROM sample_verdicts sv
        JOIN samples s ON s.id = sv.sample_id
        WHERE s.sha256 = $1
        "#,
        sha256,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| SampleVerdictError::FetchFailed { source: e }.into())
}
