use crate::error::{Result, SnapshotError};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct MachineSnapshot {
    pub id: Uuid,
    pub machine_id: i32,
    pub name: String,
    pub provider_snapshot_id: String,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
    pub guest_plugins: Option<serde_json::Value>,
    pub is_active: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
}

/// Insert a new snapshot record.
///
/// If `activate` is true, deactivates all other snapshots for this machine first.
#[allow(clippy::too_many_arguments)]
pub async fn insert_snapshot(
    pool: &PgPool,
    machine_id: i32,
    name: &str,
    provider_snapshot_id: &str,
    description: Option<&str>,
    tags: Option<&[String]>,
    guest_plugins: Option<&serde_json::Value>,
    activate: bool,
) -> Result<MachineSnapshot> {
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

/// Fetch a snapshot by machine ID and name.
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

#[derive(Debug, Clone, sqlx::FromRow, Serialize, Deserialize)]
pub struct SnapshotWithMachine {
    pub id: Uuid,
    pub machine_id: i32,
    pub machine_name: String,
    pub name: String,
    pub description: Option<String>,
    pub guest_plugins: Option<serde_json::Value>,
    pub is_active: bool,
    pub platform: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
}

pub async fn fetch_snapshots_by_platform(
    pool: &PgPool,
    platform: &str,
) -> Result<Vec<SnapshotWithMachine>> {
    sqlx::query_as::<_, SnapshotWithMachine>(
        r#"
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
        "#,
    )
    .bind(platform)
    .fetch_all(pool)
    .await
    .map_err(|e| SnapshotError::FetchFailed { source: e }.into())
}
