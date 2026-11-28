//! `machines`: analysis VMs, their resource specs, and their lifecycle status.

use crate::error::{MachineError, Result};
use malbox_config::types::Platform as MachinePlatformConfig;
use serde::{Deserialize, Serialize};
use sqlx::{AssertSqlSafe, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

/// Guest CPU architecture (`machine_arch` Postgres enum).
#[derive(sqlx::Type, Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[sqlx(type_name = "machine_arch", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "mirrors the `machine_arch` Postgres enum; matched exhaustively by malbox-resources"
)]
pub enum MachineArch {
    /// 32-bit x86.
    X86,
    /// 64-bit x86-64.
    #[default]
    X64,
}

/// Guest operating system (`machine_platform` Postgres enum).
#[derive(sqlx::Type, Debug, Serialize, Deserialize, Default, Clone, Hash, Eq, PartialEq)]
#[sqlx(type_name = "machine_platform", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "mirrors the `machine_platform` Postgres enum; matched exhaustively by \
              malbox-resources and malbox-scheduler"
)]
pub enum MachinePlatform {
    /// Microsoft Windows guest.
    #[default]
    Windows,
    /// Linux guest.
    Linux,
}

impl MachinePlatform {
    /// Canonical lowercase wire name. Matches the `machine_platform` SQL enum
    /// and the front-end `Platform` union.
    pub fn as_str(&self) -> &'static str {
        match self {
            MachinePlatform::Windows => "windows",
            MachinePlatform::Linux => "linux",
        }
    }
}

impl std::str::FromStr for MachinePlatform {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "windows" => Ok(MachinePlatform::Windows),
            "linux" => Ok(MachinePlatform::Linux),
            _ => Err(()),
        }
    }
}

/// Machine lifecycle status (`machine_status` Postgres enum).
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "machine_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "mirrors the `machine_status` Postgres enum; matched exhaustively by \
              malbox-resources reconcile"
)]
pub enum MachineStatusDb {
    /// The provider is creating the VM.
    Creating,
    /// Provisioning steps are running on the VM.
    Provisioning,
    /// Idle and available for assignment.
    Ready,
    /// Running a task.
    Assigned,
    /// Being reverted to its snapshot after a task.
    Reverting,
    /// Being torn down.
    Deleting,
    /// Unusable until startup reconcile; see `error_message`.
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

/// A row of the `machines` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct Machine {
    /// Database primary key; `None` only before insertion.
    pub id: Option<i32>,
    /// Unique machine name.
    pub name: String,
    /// Display label, initially the name.
    pub label: Option<String>,
    /// Guest CPU architecture.
    pub arch: MachineArch,
    /// Guest operating system.
    pub platform: MachinePlatform,
    /// Guest IP address reported by the provider.
    pub ip: Option<String>,
    /// Operator tags that detonation filters can require.
    pub tags: Option<Vec<String>>,
    /// Lifecycle status.
    pub status: MachineStatusDb,
    /// Image the machine was built from.
    pub image_id: Option<Uuid>,
    /// Name of the provider that manages the VM.
    pub provider: Option<String>,
    /// The provider's own identifier for the VM.
    pub provider_id: Option<String>,
    /// When the provider last reported the machine.
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_seen: Option<OffsetDateTime>,
    /// Task the machine is assigned to, while `assigned`.
    pub current_task_id: Option<i32>,
    /// Reason for the last failure, while `failed`.
    pub error_message: Option<String>,
    /// When the row was inserted.
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    /// When the row was last modified.
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
    /// Virtual CPU count.
    pub cpus: Option<i32>,
    /// Memory size in MiB.
    pub memory_mb: Option<i32>,
    /// Disk size in MiB.
    pub disk_size_mb: Option<i64>,
    /// Name of the image the machine was built from.
    pub image_name: Option<String>,
    /// Hash of the provider config the machine was built with, to detect drift.
    pub provider_config_hash: Option<String>,
}

/// Insert a new machine with full resource specs.
///
/// # Errors
///
/// Returns [`MachineError::InsertFailed`] when the insert fails, e.g. on a
/// duplicate name.
#[expect(
    clippy::too_many_arguments,
    reason = "one parameter per machine spec column"
)]
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
    tags: &[String],
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        INSERT INTO "machines" (name, label, platform, arch, image_id, image_name, cpus, memory_mb, disk_size_mb, provider_config_hash, tags, status)
        VALUES ($1, $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'creating')
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
    .bind(tags)
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

/// Replace a machine's tags.
///
/// # Errors
///
/// Returns [`MachineError::UpdateFailed`] when the update fails.
pub async fn update_machine_tags(pool: &PgPool, machine_id: i32, tags: &[String]) -> Result<()> {
    sqlx::query(r#"UPDATE "machines" SET tags = $2, updated_at = NOW() WHERE id = $1"#)
        .bind(machine_id)
        .bind(tags)
        .execute(pool)
        .await
        .map_err(|e| MachineError::UpdateFailed {
            message: "failed to update machine tags".to_string(),
            source: e,
        })?;
    Ok(())
}

/// Constraints a detonation places on the machine and snapshot it runs on.
#[derive(Debug, Clone, Default)]
pub struct MachineFilter {
    /// Required guest operating system.
    pub platform: Option<MachinePlatform>,
    /// `x86` also matches `x64` machines.
    pub arch: Option<MachineArch>,
    /// Exact machine to use.
    pub machine_id: Option<i32>,
    /// Exact snapshot to use.
    pub snapshot_id: Option<Uuid>,
    /// Snapshot name to use instead of the machine's active snapshot.
    pub snapshot_name: Option<String>,
    /// All must be present on the machine.
    pub tags: Vec<String>,
    /// All must be installed in the snapshot.
    pub required_guest_plugins: Vec<String>,
}

/// Result of [`acquire_machine_for`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    clippy::exhaustive_enums,
    reason = "closed set of acquire outcomes; matched exhaustively by malbox-resources"
)]
pub enum AcquireRow {
    /// The machine is now `assigned` to the task.
    Acquired {
        /// The assigned machine.
        machine_id: i32,
        /// Snapshot the machine must be reverted to before running.
        snapshot_id: Uuid,
    },
    /// A matching machine exists but none is `ready` right now.
    Busy,
    /// No machine could ever satisfy the filter.
    Unserviceable,
}

/// Candidate predicate shared by the acquire and existence queries. When no
/// snapshot is named, only the machine's active snapshot qualifies.
const ACQUIRE_PREDICATE: &str = r"
        ($1::machine_platform is null or m.platform = $1)
        and ($2::machine_arch is null or m.arch = $2 or ($2 = 'x86' and m.arch = 'x64'))
        and ($3::int is null or m.id = $3)
        and ($4::uuid is null or s.id = $4)
        and ($5::text is null or s.name = $5)
        and ($4::uuid is not null or $5::text is not null or s.is_active)
        and coalesce(m.tags, '{}') @> $6::varchar[]
        and coalesce(s.guest_plugins, '[]'::jsonb) @> to_jsonb($7::text[])
";

/// Atomically assign a `ready` machine matching `f` to `task_id`.
///
/// Prefers exact-arch matches, then the least recently used machine. When
/// nothing is ready, reports whether a usable match exists at all (`Busy`)
/// or not (`Unserviceable`); `failed` machines only come back at startup
/// reconcile, so they don't count.
///
/// # Errors
///
/// Returns [`MachineError::UpdateFailed`] when the transaction or the
/// claiming queries fail, and [`MachineError::FetchFailed`] when the
/// existence check fails.
pub async fn acquire_machine_for(
    pool: &PgPool,
    f: &MachineFilter,
    task_id: i32,
) -> Result<AcquireRow> {
    let update_err = |e: sqlx::Error| MachineError::UpdateFailed {
        message: "failed to acquire machine".to_string(),
        source: e,
    };
    let mut tx = pool.begin().await.map_err(update_err)?;

    let select = format!(
        "select m.id as machine_id, s.id as snapshot_id from machines m \
         join machine_snapshots s on s.machine_id = m.id \
         where m.status = 'ready' and {ACQUIRE_PREDICATE} \
         order by (m.arch is not distinct from coalesce($2, m.arch)) desc, m.updated_at asc \
         limit 1 for update of m skip locked"
    );
    let row: Option<(i32, Uuid)> = sqlx::query_as(AssertSqlSafe(select))
        .bind(f.platform.clone())
        .bind(f.arch.clone())
        .bind(f.machine_id)
        .bind(f.snapshot_id)
        .bind(f.snapshot_name.as_deref())
        .bind(&f.tags)
        .bind(&f.required_guest_plugins)
        .fetch_optional(&mut *tx)
        .await
        .map_err(update_err)?;

    if let Some((machine_id, snapshot_id)) = row {
        sqlx::query(
            "update machines set status = 'assigned', current_task_id = $1, updated_at = now() \
             where id = $2",
        )
        .bind(task_id)
        .bind(machine_id)
        .execute(&mut *tx)
        .await
        .map_err(update_err)?;
        tx.commit().await.map_err(update_err)?;
        return Ok(AcquireRow::Acquired {
            machine_id,
            snapshot_id,
        });
    }
    tx.rollback().await.map_err(update_err)?;

    let exists_sql = format!(
        "select exists(select 1 from machines m \
         join machine_snapshots s on s.machine_id = m.id \
         where m.status <> 'failed' and {ACQUIRE_PREDICATE})"
    );
    let exists: bool = sqlx::query_scalar(AssertSqlSafe(exists_sql))
        .bind(f.platform.clone())
        .bind(f.arch.clone())
        .bind(f.machine_id)
        .bind(f.snapshot_id)
        .bind(f.snapshot_name.as_deref())
        .bind(&f.tags)
        .bind(&f.required_guest_plugins)
        .fetch_one(pool)
        .await
        .map_err(|e| MachineError::FetchFailed { source: e })?;

    Ok(if exists {
        AcquireRow::Busy
    } else {
        AcquireRow::Unserviceable
    })
}

/// Release a machine after task completion, setting it to 'reverting'.
///
/// # Errors
///
/// Returns [`MachineError::UpdateFailed`] when the update fails or the
/// machine does not exist.
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
///
/// # Errors
///
/// Returns [`MachineError::UpdateFailed`] when the update fails or the
/// machine does not exist.
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
///
/// # Errors
///
/// Returns [`MachineError::UpdateFailed`] when the update fails or the
/// machine does not exist.
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
///
/// # Errors
///
/// Returns [`MachineError::FetchFailed`] when the query fails.
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
///
/// # Errors
///
/// Returns [`MachineError::FetchFailed`] when the query fails.
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
///
/// # Errors
///
/// Returns [`MachineError::UpdateFailed`] when the update fails.
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
///
/// # Errors
///
/// Returns [`MachineError::DeleteFailed`] when the delete fails.
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
