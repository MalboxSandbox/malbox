//! `provision_runs`: history of provisioner executions against machines.

use crate::error::{ProvisionRunError, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

/// Outcome of a provision run (`provision_run_status` Postgres enum).
#[derive(sqlx::Type, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "provision_run_status", rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "mirrors the `provision_run_status` Postgres enum; the variant set is fixed by \
              the schema"
)]
pub enum ProvisionRunStatus {
    /// The provisioner is still executing.
    Running,
    /// The provisioner finished successfully.
    Success,
    /// The provisioner failed or was interrupted.
    Failed,
}

/// A row of the `provision_runs` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ProvisionRun {
    /// Database primary key.
    pub id: Uuid,
    /// Machine the provisioner ran against.
    pub machine_id: i32,
    /// Name of the provisioner that ran.
    pub provisioner: String,
    /// Outcome of the run.
    pub status: ProvisionRunStatus,
    /// Provisioner configuration the run used.
    pub config: Option<serde_json::Value>,
    /// Output captured from the provisioner.
    pub output: Option<serde_json::Value>,
    /// Failure reason, for failed runs.
    pub error_message: Option<String>,
    /// Snapshot taken after a successful run.
    pub snapshot_id: Option<Uuid>,
    /// When the run started.
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    /// When the run row was last modified.
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

/// Insert a new provision run in 'running' status.
///
/// # Errors
///
/// Returns [`ProvisionRunError::InsertFailed`] when the insert fails.
pub async fn insert_provision_run(
    pool: &PgPool,
    machine_id: i32,
    provisioner: &str,
    config: Option<&serde_json::Value>,
) -> Result<ProvisionRun> {
    sqlx::query_as::<_, ProvisionRun>(
        r#"
        INSERT INTO "provision_runs" (machine_id, provisioner, config, status)
        VALUES ($1, $2, $3, 'running')
        RETURNING *
        "#,
    )
    .bind(machine_id)
    .bind(provisioner)
    .bind(config)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        ProvisionRunError::InsertFailed {
            machine_id,
            source: e,
        }
        .into()
    })
}

/// Mark a provision run as successful.
///
/// # Errors
///
/// Returns [`ProvisionRunError::UpdateFailed`] when the update fails or the
/// run does not exist.
pub async fn update_provision_run_success(
    pool: &PgPool,
    run_id: Uuid,
    output: Option<&serde_json::Value>,
    snapshot_id: Option<Uuid>,
) -> Result<ProvisionRun> {
    sqlx::query_as::<_, ProvisionRun>(
        r#"
        UPDATE "provision_runs"
        SET status = 'success', output = $1, snapshot_id = $2, updated_at = NOW()
        WHERE id = $3
        RETURNING *
        "#,
    )
    .bind(output)
    .bind(snapshot_id)
    .bind(run_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        ProvisionRunError::UpdateFailed {
            message: "failed to mark provision run as success".to_string(),
            source: e,
        }
        .into()
    })
}

/// Mark a provision run as failed.
///
/// # Errors
///
/// Returns [`ProvisionRunError::UpdateFailed`] when the update fails or the
/// run does not exist.
pub async fn update_provision_run_failed(
    pool: &PgPool,
    run_id: Uuid,
    error_message: &str,
    output: Option<&serde_json::Value>,
) -> Result<ProvisionRun> {
    sqlx::query_as::<_, ProvisionRun>(
        r#"
        UPDATE "provision_runs"
        SET status = 'failed', error_message = $1, output = $2, updated_at = NOW()
        WHERE id = $3
        RETURNING *
        "#,
    )
    .bind(error_message)
    .bind(output)
    .bind(run_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        ProvisionRunError::UpdateFailed {
            message: "failed to mark provision run as failed".to_string(),
            source: e,
        }
        .into()
    })
}

/// Fetch all provision runs for a machine, newest first.
///
/// # Errors
///
/// Returns [`ProvisionRunError::FetchFailed`] when the query fails.
pub async fn fetch_provision_runs_for_machine(
    pool: &PgPool,
    machine_id: i32,
) -> Result<Vec<ProvisionRun>> {
    sqlx::query_as::<_, ProvisionRun>(
        r#"
        SELECT * FROM "provision_runs"
        WHERE machine_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(machine_id)
    .fetch_all(pool)
    .await
    .map_err(|e| ProvisionRunError::FetchFailed { source: e }.into())
}

/// Delete all provision runs that reference a given snapshot.
///
/// # Errors
///
/// Returns [`ProvisionRunError::DeleteFailed`] when the delete fails.
pub async fn delete_provision_runs_for_snapshot(pool: &PgPool, snapshot_id: Uuid) -> Result<u64> {
    let result = sqlx::query(
        r#"
        DELETE FROM "provision_runs" WHERE snapshot_id = $1
        "#,
    )
    .bind(snapshot_id)
    .execute(pool)
    .await
    .map_err(|e| ProvisionRunError::DeleteFailed { source: e })?;
    Ok(result.rows_affected())
}

/// Mark all 'running' provision runs as 'failed' (interrupted by restart).
///
/// # Errors
///
/// Returns [`ProvisionRunError::UpdateFailed`] when the update fails.
pub async fn mark_interrupted_runs(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query(
        r#"
        UPDATE "provision_runs"
        SET status = 'failed', error_message = 'Interrupted by restart', updated_at = NOW()
        WHERE status = 'running'
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| ProvisionRunError::UpdateFailed {
        message: "failed to mark interrupted runs".to_string(),
        source: e,
    })?;
    Ok(result.rows_affected())
}
