//! `machine_snapshots`: provider snapshots of machines, at most one active per machine.

use crate::error::{Result, SnapshotError};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

/// A row of the `machine_snapshots` table.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct MachineSnapshot {
    /// Database primary key.
    pub id: Uuid,
    /// Machine the snapshot was taken of.
    pub machine_id: i32,
    /// Snapshot name, unique per machine.
    pub name: String,
    /// The provider's own identifier for the snapshot.
    pub provider_snapshot_id: String,
    /// Free-form description.
    pub description: Option<String>,
    /// Operator tags.
    pub tags: Option<Vec<String>>,
    /// Guest plugins installed in the snapshot, as a JSON array of names.
    pub guest_plugins: Option<serde_json::Value>,
    /// Whether this is the machine's default snapshot for detonations.
    pub is_active: bool,
    /// When the row was inserted.
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    /// When the row was last modified.
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

/// Whether [`insert_snapshot`] makes the new snapshot its machine's active one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SnapshotActivation {
    /// Deactivate the machine's other snapshots and mark the new one active.
    Activate,
    /// Insert the snapshot inactive, leaving the current active one in place.
    Inactive,
}

/// Insert a new snapshot record.
///
/// With [`SnapshotActivation::Activate`], deactivates all other snapshots for
/// this machine first.
///
/// # Errors
///
/// Returns [`SnapshotError::UpdateFailed`] when deactivating the other
/// snapshots fails, and [`SnapshotError::InsertFailed`] when the insert fails.
#[expect(
    clippy::too_many_arguments,
    reason = "one parameter per snapshot column"
)]
pub async fn insert_snapshot(
    pool: &PgPool,
    machine_id: i32,
    name: &str,
    provider_snapshot_id: &str,
    description: Option<&str>,
    tags: Option<&[String]>,
    guest_plugins: Option<&serde_json::Value>,
    activation: SnapshotActivation,
) -> Result<MachineSnapshot> {
    let activate = activation == SnapshotActivation::Activate;
    if activate {
        deactivate_all(pool, machine_id).await?;
    }

    let plugins_val = guest_plugins
        .cloned()
        .unwrap_or_else(|| serde_json::Value::Array(vec![]));

    sqlx::query_as::<_, MachineSnapshot>(
        r#"
        INSERT INTO "machine_snapshots"
            (machine_id, name, provider_snapshot_id, description, tags, guest_plugins, is_active)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING *
        "#,
    )
    .bind(machine_id)
    .bind(name)
    .bind(provider_snapshot_id)
    .bind(description)
    .bind(tags)
    .bind(plugins_val)
    .bind(activate)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        SnapshotError::InsertFailed {
            name: name.to_string(),
            machine_id,
            source: e,
        }
        .into()
    })
}

/// Fetch the active snapshot for a machine.
///
/// # Errors
///
/// Returns [`SnapshotError::FetchFailed`] when the query fails.
pub async fn fetch_active_snapshot(
    pool: &PgPool,
    machine_id: i32,
) -> Result<Option<MachineSnapshot>> {
    sqlx::query_as::<_, MachineSnapshot>(
        r#"
        SELECT * FROM "machine_snapshots"
        WHERE machine_id = $1 AND is_active = true
        LIMIT 1
        "#,
    )
    .bind(machine_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| SnapshotError::FetchFailed { source: e }.into())
}

/// Fetch all snapshots for a machine, ordered by creation time.
///
/// # Errors
///
/// Returns [`SnapshotError::FetchFailed`] when the query fails.
pub async fn fetch_snapshots_for_machine(
    pool: &PgPool,
    machine_id: i32,
) -> Result<Vec<MachineSnapshot>> {
    sqlx::query_as::<_, MachineSnapshot>(
        r#"
        SELECT * FROM "machine_snapshots"
        WHERE machine_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(machine_id)
    .fetch_all(pool)
    .await
    .map_err(|e| SnapshotError::FetchFailed { source: e }.into())
}

/// Activate a specific snapshot (deactivates all others for that machine).
///
/// # Errors
///
/// Returns [`SnapshotError::FetchFailed`] when the snapshot does not exist or
/// cannot be read, and [`SnapshotError::UpdateFailed`] when changing the
/// active flags fails.
pub async fn activate_snapshot(pool: &PgPool, snapshot_id: Uuid) -> Result<MachineSnapshot> {
    let snapshot =
        sqlx::query_as::<_, MachineSnapshot>(r#"SELECT * FROM "machine_snapshots" WHERE id = $1"#)
            .bind(snapshot_id)
            .fetch_one(pool)
            .await
            .map_err(|e| SnapshotError::FetchFailed { source: e })?;

    deactivate_all(pool, snapshot.machine_id).await?;

    sqlx::query_as::<_, MachineSnapshot>(
        r#"
        UPDATE "machine_snapshots"
        SET is_active = true
        WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(snapshot_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        SnapshotError::UpdateFailed {
            message: "failed to activate snapshot".to_string(),
            source: e,
        }
        .into()
    })
}

/// Deactivate all snapshots for a machine.
async fn deactivate_all(pool: &PgPool, machine_id: i32) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE "machine_snapshots"
        SET is_active = false
        WHERE machine_id = $1 AND is_active = true
        "#,
    )
    .bind(machine_id)
    .execute(pool)
    .await
    .map_err(|e| SnapshotError::UpdateFailed {
        message: "failed to deactivate snapshots".to_string(),
        source: e,
    })?;
    Ok(())
}

/// Fetch a snapshot by ID.
///
/// # Errors
///
/// Returns [`SnapshotError::FetchFailed`] when the query fails.
pub async fn fetch_snapshot_by_id(pool: &PgPool, id: Uuid) -> Result<Option<MachineSnapshot>> {
    sqlx::query_as::<_, MachineSnapshot>(r#"SELECT * FROM "machine_snapshots" WHERE id = $1"#)
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| SnapshotError::FetchFailed { source: e }.into())
}

/// Fetch a snapshot by machine ID and name.
///
/// # Errors
///
/// Returns [`SnapshotError::FetchFailed`] when the query fails.
pub async fn fetch_snapshot_by_name(
    pool: &PgPool,
    machine_id: i32,
    name: &str,
) -> Result<Option<MachineSnapshot>> {
    sqlx::query_as::<_, MachineSnapshot>(
        r#"
        SELECT * FROM "machine_snapshots"
        WHERE machine_id = $1 AND name = $2
        LIMIT 1
        "#,
    )
    .bind(machine_id)
    .bind(name)
    .fetch_optional(pool)
    .await
    .map_err(|e| SnapshotError::FetchFailed { source: e }.into())
}

/// Delete a single snapshot by ID.
///
/// # Errors
///
/// Returns [`SnapshotError::DeleteFailed`] when the delete fails.
pub async fn delete_snapshot(pool: &PgPool, snapshot_id: Uuid) -> Result<Option<MachineSnapshot>> {
    sqlx::query_as::<_, MachineSnapshot>(
        r#"
        DELETE FROM "machine_snapshots" WHERE id = $1
        RETURNING *
        "#,
    )
    .bind(snapshot_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| SnapshotError::DeleteFailed { source: e }.into())
}

/// Delete all snapshots for a machine.
///
/// # Errors
///
/// Returns [`SnapshotError::DeleteFailed`] when the delete fails.
pub async fn delete_snapshots_for_machine(pool: &PgPool, machine_id: i32) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM "machine_snapshots" WHERE machine_id = $1
        "#,
    )
    .bind(machine_id)
    .execute(pool)
    .await
    .map_err(|e| SnapshotError::DeleteFailed { source: e })?;
    Ok(())
}

/// A snapshot joined with the name and platform of its machine.
#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct SnapshotWithMachine {
    /// Snapshot primary key.
    pub id: Uuid,
    /// Machine the snapshot was taken of.
    pub machine_id: i32,
    /// Name of that machine.
    pub machine_name: String,
    /// Snapshot name.
    pub name: String,
    /// Free-form snapshot description.
    pub description: Option<String>,
    /// Guest plugins installed in the snapshot, as a JSON array of names.
    pub guest_plugins: Option<serde_json::Value>,
    /// Whether this is the machine's active snapshot.
    pub is_active: bool,
    /// The machine's `machine_platform` label, e.g. `windows`.
    pub platform: String,
    /// When the snapshot row was inserted.
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
}

/// Snapshots of every machine on `platform` (a `machine_platform` label), newest first.
///
/// # Errors
///
/// Returns [`SnapshotError::FetchFailed`] when the query fails.
pub async fn fetch_snapshots_by_platform(
    pool: &PgPool,
    platform: &str,
) -> Result<Vec<SnapshotWithMachine>> {
    sqlx::query_as::<_, SnapshotWithMachine>(
        r"
        SELECT
            ms.id,
            ms.machine_id,
            m.name AS machine_name,
            ms.name,
            ms.description,
            ms.guest_plugins,
            ms.is_active,
            m.platform::text AS platform,
            ms.created_at
        FROM machine_snapshots ms
        JOIN machines m ON ms.machine_id = m.id
        WHERE m.platform::text = $1
        ORDER BY ms.created_at DESC
        ",
    )
    .bind(platform)
    .fetch_all(pool)
    .await
    .map_err(|e| SnapshotError::FetchFailed { source: e }.into())
}
