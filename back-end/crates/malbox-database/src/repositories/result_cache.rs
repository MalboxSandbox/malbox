//! `result_cache`: host-job result reuse keyed by a content/config hash.

use crate::error::{Result, ResultCacheError};
use sqlx::PgPool;

/// Look up a live cache entry: `(job_id, empty)`.
pub async fn lookup(pool: &PgPool, key: &str) -> Result<Option<(i64, bool)>> {
    sqlx::query_as::<_, (i64, bool)>(
        "select job_id, empty from result_cache where cache_key = $1 and expires_on > now()",
    )
    .bind(key)
    .fetch_optional(pool)
    .await
    .map_err(|source| {
        ResultCacheError::FetchFailed {
            target: format!("cache entry {key}"),
            source,
        }
        .into()
    })
}

/// Insert or replace the entry for `key`, valid for `ttl_secs`.
pub async fn store(
    pool: &PgPool,
    key: &str,
    job_id: i64,
    empty: bool,
    ttl_secs: i64,
) -> Result<()> {
    sqlx::query(
        "insert into result_cache (cache_key, job_id, empty, expires_on) \
         values ($1, $2, $3, now() + $4::bigint * interval '1 second') \
         on conflict (cache_key) do update set job_id = excluded.job_id, empty = excluded.empty, \
         created_on = now(), expires_on = excluded.expires_on",
    )
    .bind(key)
    .bind(job_id)
    .bind(empty)
    .bind(ttl_secs)
    .execute(pool)
    .await
    .map_err(|source| ResultCacheError::InsertFailed {
        target: format!("cache entry {key} for job {job_id}"),
        source,
    })?;
    Ok(())
}

/// Delete expired entries. Returns the number removed.
pub async fn purge_expired(pool: &PgPool) -> Result<u64> {
    let r = sqlx::query("delete from result_cache where expires_on <= now()")
        .execute(pool)
        .await
        .map_err(|source| ResultCacheError::DeleteFailed {
            target: "expired cache entries".to_string(),
            source,
        })?;
    Ok(r.rows_affected())
}
