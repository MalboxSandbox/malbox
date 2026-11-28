//! `samples`: deduplicated submitted files, identified by their hashes.

use crate::error::{Result, SampleError};
use sqlx::{AssertSqlSafe, FromRow, PgPool, postgres::PgDatabaseError, query_as};
use time::OffsetDateTime;

/// Hashes and metadata of a file to register as a sample.
#[derive(Debug, Clone)]
pub struct Sample {
    /// File size in bytes.
    pub file_size: i64,
    /// Detected file type description.
    pub file_type: String,
    /// Hex MD5 digest.
    pub md5: String,
    /// Hex CRC32 checksum.
    pub crc32: String,
    /// Hex SHA-1 digest.
    pub sha1: String,
    /// Hex SHA-256 digest.
    pub sha256: String,
    /// Hex SHA-512 digest.
    pub sha512: String,
    /// ssdeep fuzzy hash.
    pub ssdeep: String,
}

/// A row of the `samples` table.
#[derive(FromRow, Debug, Clone)]
pub struct SampleEntity {
    /// Database primary key.
    pub id: i64,
    /// File size in bytes.
    pub file_size: i64,
    /// Detected file type description.
    pub file_type: String,
    /// Hex MD5 digest.
    pub md5: String,
    /// Hex CRC32 checksum.
    pub crc32: String,
    /// Hex SHA-1 digest.
    pub sha1: String,
    /// Hex SHA-256 digest.
    pub sha256: String,
    /// Hex SHA-512 digest.
    pub sha512: String,
    /// ssdeep fuzzy hash.
    pub ssdeep: String,
    /// When the row was inserted.
    pub created_at: OffsetDateTime,
    /// When the row was last modified.
    pub updated_at: OffsetDateTime,
}

impl Default for SampleEntity {
    fn default() -> Self {
        SampleEntity {
            id: 1,
            file_size: 2048,
            file_type: String::from("Default SampleEntity"),
            md5: String::from("none"),
            crc32: String::from("none"),
            sha1: String::from("none"),
            sha256: String::from("none"),
            sha512: String::from("none"),
            ssdeep: String::from("none"),
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
        }
    }
}

/// Insert a sample, or return the existing row when one with the same hashes exists.
///
/// # Errors
///
/// Returns [`SampleError::InsertFailed`] when the insert fails for a reason
/// other than a duplicate, and [`SampleError::FetchFailed`] when looking up
/// the existing duplicate fails.
pub async fn insert_sample(pool: &PgPool, sample: Sample) -> Result<SampleEntity> {
    match query_as!(
        SampleEntity,
        r#"
        INSERT INTO "samples" (file_size, file_type, md5, crc32, sha1, sha256, sha512, ssdeep)
        VALUES ($1::bigint, $2::varchar, $3::varchar, $4::varchar, $5::varchar, $6::varchar, $7::varchar, $8::varchar)
        RETURNING *
        "#,
        sample.file_size,
        sample.file_type,
        sample.md5,
        sample.crc32,
        sample.sha1,
        sample.sha256,
        sample.sha512,
        sample.ssdeep
    )
    .fetch_one(pool)
    .await
    {
        Ok(new_sample) => Ok(new_sample),
        Err(e) => {
            if let Some(db_error) = e.as_database_error() {
                let pg_error = db_error.downcast_ref::<PgDatabaseError>();
                    if pg_error.code() == "23505" {
                        let existing_sample = query_as!(
                            SampleEntity,
                            r#"
                            SELECT * FROM "samples"
                            WHERE md5 = $1 AND crc32 = $2 AND sha1 = $3 AND sha256 = $4 AND sha512 = $5
                            "#,
                            sample.md5,
                            sample.crc32,
                            sample.sha1,
                            sample.sha256,
                            sample.sha512
                        )
                        .fetch_one(pool)
                        .await
                        .map_err(|e| SampleError::FetchFailed { hash: sample.sha256, message: "Failed to fetch existing sample".to_string(), source: e })?;

                        return Ok(existing_sample);
                    }
                }

            Err(SampleError::InsertFailed { hash: String::new(), message: String::new(), source: e }.into())
        }
    }
}

/// Fetch a sample by one of its hashes; `hash_type` is `md5`, `sha1`, `sha256` or `sha512`.
///
/// # Errors
///
/// Returns [`SampleError::FetchFailed`] when `hash_type` is not one of the
/// supported names or the query fails.
pub async fn fetch_sample_by_hash(
    pool: &PgPool,
    hash_type: &str,
    hash_value: &str,
) -> Result<Option<SampleEntity>> {
    let column = match hash_type {
        "md5" => "md5",
        "sha1" => "sha1",
        "sha256" => "sha256",
        "sha512" => "sha512",
        _ => {
            return Err(SampleError::FetchFailed {
                hash: hash_value.to_string(),
                message: format!("Unsupported hash type: {hash_type}"),
                source: sqlx::Error::Protocol(format!("unsupported hash type: {hash_type}")),
            }
            .into());
        }
    };

    let query = format!(
        r#"SELECT id::bigint, file_size::bigint, file_type, md5, crc32, sha1, sha256, sha512, ssdeep, created_at, updated_at FROM "samples" WHERE {column} = $1"#
    );

    // Safety: `column` is restricted to the whitelist above and the hash value
    // is bound as a parameter, so the dynamic SQL is injection-safe.
    sqlx::query_as::<_, SampleEntity>(AssertSqlSafe(query))
        .bind(hash_value)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            SampleError::FetchFailed {
                hash: hash_value.to_string(),
                message: format!("Failed to fetch sample by {hash_type}"),
                source: e,
            }
            .into()
        })
}

/// Fetch a sample by primary key, `None` when it does not exist.
///
/// # Errors
///
/// Returns [`SampleError::FetchFailed`] when the query fails.
pub async fn fetch_sample_by_id(pool: &PgPool, id: i64) -> Result<Option<SampleEntity>> {
    // `samples.id` is an `integer` column: an id outside `i32` cannot exist.
    let Ok(id_i32) = i32::try_from(id) else {
        return Ok(None);
    };
    query_as!(
        SampleEntity,
        r#"SELECT * FROM "samples" WHERE id = $1"#,
        id_i32
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        SampleError::FetchFailed {
            hash: id.to_string(),
            message: "Failed to fetch sample by ID".to_string(),
            source: e,
        }
        .into()
    })
}

/// Batch-load samples by id. Used by the task list to attach sample info
/// without an N+1 (one query per page instead of one per row).
/// Returns only the rows that exist; callers map by id.
///
/// # Errors
///
/// Returns [`SampleError::FetchFailed`] when the query fails.
pub async fn fetch_samples_by_ids(pool: &PgPool, ids: &[i64]) -> Result<Vec<SampleEntity>> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    // `samples.id` is an `integer` column: ids outside `i32` cannot match.
    let ids_i32: Vec<i32> = ids.iter().filter_map(|&i| i32::try_from(i).ok()).collect();
    query_as!(
        SampleEntity,
        r#"SELECT * FROM "samples" WHERE id = ANY($1)"#,
        &ids_i32
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        SampleError::FetchFailed {
            hash: String::new(),
            message: "Failed to batch-fetch samples by ids".to_string(),
            source: e,
        }
        .into()
    })
}
