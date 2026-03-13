use crate::error::{MachineError, Result};
use malbox_config::types::Platform as MachinePlatformConfig;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(sqlx::Type, Debug, Clone, Serialize, Deserialize, Default)]
#[sqlx(type_name = "machine_arch", rename_all = "lowercase")]
pub enum MachineArch {
    X86,
    #[default]
    X64,
}

#[derive(sqlx::Type, Debug, Serialize, Deserialize, Default, Clone, Hash, Eq, PartialEq)]
#[sqlx(type_name = "machine_platform", rename_all = "lowercase")]
pub enum MachinePlatform {
    #[default]
    Windows,
    Linux,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "machine_status", rename_all = "lowercase")]
pub enum MachineStatusDb {
    Creating,
    Provisioning,
    Ready,
    Assigned,
    Reverting,
    Deleting,
    Failed,
}

impl From<MachinePlatformConfig> for MachinePlatform {
    fn from(value: MachinePlatformConfig) -> Self {
        match value {
            MachinePlatformConfig::Linux => MachinePlatform::Linux,
            MachinePlatformConfig::Windows => MachinePlatform::Windows,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Machine {
    pub id: Option<i32>,
    pub name: String,
    pub label: Option<String>,
    pub arch: MachineArch,
    pub platform: MachinePlatform,
    pub ip: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: MachineStatusDb,
    pub image_id: Option<Uuid>,
    pub clean_snapshot: Option<String>,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub provisioned: bool,
    pub provisioner: Option<String>,
    pub provision_output: Option<serde_json::Value>,
    pub last_seen: Option<OffsetDateTime>,
    pub current_task_id: Option<i32>,
    pub error_message: Option<String>,
    pub created_at: Option<OffsetDateTime>,
    pub updated_at: Option<OffsetDateTime>,
}

/// Insert a new machine in 'creating' status.
pub async fn insert_machine(
    pool: &PgPool,
    name: &str,
    platform: MachinePlatform,
    arch: MachineArch,
    image_id: Uuid,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        INSERT INTO "machines" (name, label, platform, arch, image_id, status)
        VALUES ($1, $1, $2, $3, $4, 'creating')
        RETURNING *
        "#,
    )
    .bind(name)
    .bind(platform)
    .bind(arch)
    .bind(image_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::InsertFailed {
            name: name.to_string(),
            message: "failed to insert machine record".to_string(),
            source: e,
        }
        .into()
    })
}

/// Atomically acquire a 'ready' machine for a task.
/// Uses FOR UPDATE SKIP LOCKED to avoid contention.
/// Sets status to 'assigned' and records the task_id.
pub async fn acquire_machine(
    pool: &PgPool,
    platform: MachinePlatform,
    task_id: i32,
) -> Result<Option<Machine>> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET status = 'assigned',
            current_task_id = $1,
            updated_at = NOW()
        WHERE id = (
            SELECT id FROM "machines"
            WHERE status = 'ready'
              AND platform = $2
            ORDER BY updated_at ASC
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING *
        "#,
    )
    .bind(task_id)
    .bind(platform)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "failed to acquire machine".to_string(),
            source: e,
        }
        .into()
    })
}

/// Release a machine after task completion, setting it to 'reverting'.
pub async fn release_machine(pool: &PgPool, machine_id: i32) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET status = 'reverting',
            current_task_id = NULL,
            updated_at = NOW()
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(machine_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "failed to release machine".to_string(),
            source: e,
        }
        .into()
    })
}

/// Update the lifecycle status of a machine, with an optional error message.
pub async fn update_machine_status(
    pool: &PgPool,
    machine_id: i32,
    status: MachineStatusDb,
    error_message: Option<&str>,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET status = $1,
            error_message = $2,
            updated_at = NOW()
        WHERE id = $3
        RETURNING *
        "#,
    )
    .bind(status)
    .bind(error_message)
    .bind(machine_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "failed to update machine status".to_string(),
            source: e,
        }
        .into()
    })
}

/// Set provider-specific information after a machine has been created by a provider.
pub async fn set_machine_provider_info(
    pool: &PgPool,
    machine_id: i32,
    provider: &str,
    provider_id: &str,
    ip: &str,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET provider = $1,
            provider_id = $2,
            ip = $3,
            last_seen = NOW(),
            updated_at = NOW()
        WHERE id = $4
        RETURNING *
        "#,
    )
    .bind(provider)
    .bind(provider_id)
    .bind(ip)
    .bind(machine_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "failed to set machine provider info".to_string(),
            source: e,
        }
        .into()
    })
}

/// Record provisioning details for a machine.
///
/// The output string is stored as JSONB. If it's valid JSON it's stored directly;
/// otherwise it's wrapped as `{"raw": "..."}`.
pub async fn set_machine_provision_info(
    pool: &PgPool,
    machine_id: i32,
    provisioner: &str,
    output: Option<&str>,
) -> Result<Machine> {
    let json_output: Option<serde_json::Value> = output.map(|s| {
        serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({ "raw": s }))
    });

    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET provisioned = true,
            provisioner = $1,
            provision_output = $2,
            updated_at = NOW()
        WHERE id = $3
        RETURNING *
        "#,
    )
    .bind(provisioner)
    .bind(json_output)
    .bind(machine_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "failed to set machine provision info".to_string(),
            source: e,
        }
        .into()
    })
}

/// Record the clean snapshot name for a machine.
pub async fn set_machine_snapshot(
    pool: &PgPool,
    machine_id: i32,
    snapshot_name: &str,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET clean_snapshot = $1,
            updated_at = NOW()
        WHERE id = $2
        RETURNING *
        "#,
    )
    .bind(snapshot_name)
    .bind(machine_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "failed to set machine snapshot".to_string(),
            source: e,
        }
        .into()
    })
}

/// Fetch a single machine by ID.
pub async fn fetch_machine(pool: &PgPool, machine_id: i32) -> Result<Option<Machine>> {
    sqlx::query_as::<_, Machine>(
        r#"
        SELECT * FROM "machines" WHERE id = $1
        "#,
    )
    .bind(machine_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| MachineError::FetchFailed { source: e }.into())
}

/// Fetch all machines, ordered by ID.
pub async fn fetch_all_machines(pool: &PgPool) -> Result<Vec<Machine>> {
    sqlx::query_as::<_, Machine>(
        r#"
        SELECT * FROM "machines" ORDER BY id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| MachineError::FetchFailed { source: e }.into())
}

/// Delete a machine by ID.
pub async fn delete_machine(pool: &PgPool, machine_id: i32) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM "machines" WHERE id = $1
        "#,
    )
    .bind(machine_id)
    .execute(pool)
    .await
    .map_err(|e| MachineError::DeleteFailed { source: e })?;
    Ok(())
}
