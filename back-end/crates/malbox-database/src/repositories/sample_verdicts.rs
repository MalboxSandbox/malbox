use crate::error::{Result, SampleVerdictError};
use crate::repositories::plugin_reports::DbClassification;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;

/// Row from the `sample_verdicts` table.
#[derive(FromRow, Debug, Clone)]
pub struct SampleVerdict {
    pub sample_id: i64,
    pub classification: Option<DbClassification>,
    pub score: Option<i16>,
    pub indicator_count: i32,
    pub ttp_count: i32,
    pub plugin_names: Vec<String>,
    pub task_count: i32,
    pub last_task_id: Option<i32>,
    pub updated_on: OffsetDateTime,
}

pub struct UpsertSampleVerdict<'a> {
    pub sample_id: i64,
    pub classification: Option<DbClassification>,
    pub score: Option<i16>,
    pub indicator_count: i32,
    pub ttp_count: i32,
    pub plugin_names: &'a [String],
    pub task_count: i32,
    pub last_task_id: Option<i32>,
}

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
