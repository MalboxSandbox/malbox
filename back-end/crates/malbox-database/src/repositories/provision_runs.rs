use crate::error::{ProvisionRunError, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(sqlx::Type, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[sqlx(type_name = "provision_run_status", rename_all = "lowercase")]
pub enum ProvisionRunStatus {
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct ProvisionRun {
    pub id: Uuid,
    pub machine_id: i32,
    pub provisioner: String,
    pub status: ProvisionRunStatus,
    pub config: Option<serde_json::Value>,
    pub output: Option<serde_json::Value>,
    pub error_message: Option<String>,
    pub snapshot_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

/// Insert a new provision run in 'running' status.
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

/// Mark all 'running' provision runs as 'failed' (interrupted by restart).
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
