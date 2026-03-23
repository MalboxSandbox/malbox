use crate::error::{MachineError, Result};
use malbox_config::types::Platform as MachinePlatformConfig;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(sqlx::Type, Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
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

impl From<malbox_config::Arch> for MachineArch {
    fn from(a: malbox_config::Arch) -> Self {
        match a {
            malbox_config::Arch::X64 => MachineArch::X64,
            malbox_config::Arch::X86 => MachineArch::X86,
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
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_seen: Option<OffsetDateTime>,
    pub current_task_id: Option<i32>,
    pub error_message: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
    pub cpus: Option<i32>,
    pub memory_mb: Option<i32>,
    pub disk_size_mb: Option<i64>,
    pub image_name: Option<String>,
    pub provider_config_hash: Option<String>,
}

/// Insert a new machine with full resource specs.
pub async fn insert_machine(
    pool: &PgPool,
    name: &str,
    platform: MachinePlatform,
    arch: MachineArch,
    image_id: Uuid,
    image_name: &str,
    cpus: i32,
    memory_mb: i32,
    disk_size_mb: i64,
    provider_config_hash: Option<&str>,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        INSERT INTO "machines" (name, label, platform, arch, image_id, image_name, cpus, memory_mb, disk_size_mb, provider_config_hash, status)
        VALUES ($1, $1, $2, $3, $4, $5, $6, $7, $8, $9, 'creating')
        RETURNING *
        "#,
    )
    .bind(name)
    .bind(platform)
    .bind(arch)
    .bind(image_id)
    .bind(image_name)
    .bind(cpus)
    .bind(memory_mb)
    .bind(disk_size_mb)
    .bind(provider_config_hash)
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

/// Atomically transition a machine from Ready to Provisioning.
/// Returns None if the machine is not in Ready status (prevents races).
pub async fn transition_to_provisioning(pool: &PgPool, machine_id: i32) -> Result<Option<Machine>> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET status = 'provisioning',
            error_message = NULL,
            updated_at = NOW()
        WHERE id = $1 AND status = 'ready'
        RETURNING *
        "#,
    )
    .bind(machine_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "failed to transition to provisioning".to_string(),
            source: e,
        }
        .into()
    })
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
