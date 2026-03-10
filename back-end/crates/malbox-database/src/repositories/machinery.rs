use crate::error::{MachineError, Result};
use malbox_config::types::Platform as MachinePlatformConfig;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Postgres, QueryBuilder};
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use uuid::Uuid;

#[derive(sqlx::Type, Debug, Serialize, Deserialize, Default)]
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

#[derive(sqlx::Type, Debug, Serialize, Deserialize, Default, Clone, PartialEq, Eq)]
#[sqlx(type_name = "provision_status", rename_all = "lowercase")]
pub enum ProvisionStatusDb {
    #[default]
    Unprovisioned,
    Provisioning,
    Provisioned,
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

#[derive(Default, FromRow, Debug)]
pub struct Machine {
    pub id: Option<i32>,
    pub name: String,
    pub label: String,
    pub arch: MachineArch,
    pub platform: MachinePlatform,
    pub ip: String,
    pub interface: Option<String>,
    pub tags: Option<Vec<String>>,
    pub snapshot: Option<String>,
    pub locked: bool,
    pub locked_changed_on: Option<PrimitiveDateTime>,
    pub status: Option<String>,
    pub status_changed_on: Option<PrimitiveDateTime>,
    pub reserved: bool,
    // Pool persistence fields
    pub image_id: Option<Uuid>,
    pub provision_status: Option<ProvisionStatusDb>,
    pub clean_snapshot: Option<String>,
    pub pool_member: Option<bool>,
    pub provider: Option<String>,
    pub provider_id: Option<String>,
    pub last_seen: Option<OffsetDateTime>,
}

#[derive(Default)]
pub struct MachineFilter {
    pub locked: Option<bool>,
    pub label: Option<String>,
    pub platform: Option<MachinePlatform>,
    pub tags: Option<String>,
    pub arch: Option<MachineArch>,
    pub include_reserved: bool,
    pub os_version: Option<String>,
}

pub async fn insert_machine(pool: &PgPool, machine: Machine) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        INSERT into "machines" (
            name, label, arch, platform, ip, interface, tags,
            snapshot, locked, locked_changed_on, status, status_changed_on,
            reserved, image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
            $14, $15, $16, $17, $18, $19, $20
        )
        RETURNING
            id, name, label, arch, platform,
            ip, interface, tags, snapshot, locked, locked_changed_on, status,
            status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        "#,
    )
    .bind(machine.name.clone())
    .bind(machine.label.clone())
    .bind(machine.arch)
    .bind(machine.platform)
    .bind(machine.ip.clone())
    .bind(machine.interface.clone())
    .bind(machine.tags.clone())
    .bind(machine.snapshot.clone())
    .bind(machine.locked)
    .bind(machine.locked_changed_on)
    .bind(machine.status.clone())
    .bind(machine.status_changed_on)
    .bind(machine.reserved)
    .bind(machine.image_id)
    .bind(machine.provision_status)
    .bind(machine.clean_snapshot.clone())
    .bind(machine.pool_member)
    .bind(machine.provider.clone())
    .bind(machine.provider_id.clone())
    .bind(machine.last_seen)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::InsertFailed {
            name: machine.name,
            message: "failed to insert machine record".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn clean_machines(pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        TRUNCATE "machines";
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| MachineError::TruncateFailed { source: e });

    sqlx::query(
        r#"
        DELETE FROM "machines";
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| MachineError::DeleteFailed { source: e });

    Ok(())
}

pub async fn fetch_machines(pool: &PgPool, filter: Option<MachineFilter>) -> Result<Vec<Machine>> {
    // the query will be adjusted depending on other params to filter out specific machines

    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT
            id, name, label, arch, platform,
            ip, interface, tags, snapshot, locked, locked_changed_on, status,
            status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        FROM "machines"
        "#,
    );

    if let Some(filter) = filter {
        if let Some(locked) = filter.locked {
            query_builder.push(" AND locked = ");
            query_builder.push_bind(locked);
        }
        if let Some(label) = filter.label {
            query_builder.push(" AND label = ");
            query_builder.push_bind(label);
        }
        if let Some(platform) = filter.platform {
            query_builder.push(" AND platform = ");
            query_builder.push_bind(platform);
        }
        if let Some(tags) = filter.tags {
            query_builder.push(" AND tags @> ");
            query_builder.push_bind(tags);
        }
        if let Some(arch) = filter.arch {
            query_builder.push(" AND arch = ");
            query_builder.push_bind(arch);
        }
        // if let Some(os_version) = filter.os_version {
        //     query_builder.push(" AND os_version = ");
        //     query_builder.push_bind(os_version);
        // }
        if !filter.include_reserved {
            query_builder.push(" AND reserved = false");
        }
    }

    let query = query_builder
        .build_query_as::<Machine>()
        .fetch_all(pool)
        .await
        .map_err(|e| MachineError::FetchFailed { source: e })?;

    Ok(query)
}

pub async fn fetch_machine(
    pool: &PgPool,
    filter: Option<MachineFilter>,
) -> Result<Option<Machine>> {
    // the query will be adjusted depending on other params to filter out specific machines

    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT
            id, name, label, arch, platform,
            ip, interface, tags, snapshot, locked, locked_changed_on, status,
            status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        FROM "machines" WHERE 1 = 1
        "#,
    );

    if let Some(filter) = filter {
        if let Some(locked) = filter.locked {
            query_builder.push(" AND locked = ");
            query_builder.push_bind(locked);
        }
        if let Some(label) = filter.label {
            query_builder.push(" AND label = ");
            query_builder.push_bind(label);
        }
        if let Some(platform) = filter.platform {
            query_builder.push(" AND platform = ");
            query_builder.push_bind(platform);
        }
        if let Some(tags) = filter.tags {
            query_builder.push(" AND tags @> ");
            query_builder.push_bind(tags);
        }
        if let Some(arch) = filter.arch {
            query_builder.push(" AND arch = ");
            query_builder.push_bind(arch);
        }
        // if let Some(os_version) = filter.os_version {
        //     query_builder.push(" AND os_version = ");
        //     query_builder.push_bind(os_version);
        // }
        if !filter.include_reserved {
            query_builder.push(" AND reserved = false");
        }
    }

    let query = query_builder
        .build_query_as::<Machine>()
        .fetch_optional(pool)
        .await
        .map_err(|e| MachineError::FetchFailed { source: e })?;

    Ok(query)
}

pub async fn update_machine(pool: &PgPool, id: i32, machine: Machine) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET
            name = $1,
            label = $2,
            arch = $3,
            platform = $4,
            ip = $5,
            interface = $6,
            tags = $7,
            snapshot = $8,
            locked = $9,
            locked_changed_on = $10,
            status = $11,
            status_changed_on = $12,
            reserved = $13
        WHERE id = $14
        RETURNING
            id, name, label, arch, platform,
            ip, interface, tags, snapshot, locked, locked_changed_on, status,
            status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        "#,
    )
    .bind(machine.name.clone())
    .bind(machine.label.clone())
    .bind(machine.arch)
    .bind(machine.platform)
    .bind(machine.ip.clone())
    .bind(machine.interface.clone())
    .bind(machine.tags.clone())
    .bind(machine.snapshot.clone())
    .bind(machine.locked)
    .bind(machine.locked_changed_on)
    .bind(machine.status.clone())
    .bind(machine.status_changed_on)
    .bind(machine.reserved)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "Failed to update machine".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn update_machine_status(
    pool: &PgPool,
    id: i32,
    locked: bool,
    status: Option<&str>,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET
            locked = $1,
            locked_changed_on = NOW(),
            status = $2,
            status_changed_on = NOW()
        WHERE id = $3
        RETURNING
            id, name, label, arch, platform,
            ip, interface, tags, snapshot, locked, locked_changed_on, status,
            status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        "#,
    )
    .bind(locked)
    .bind(status)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "Failed to update status".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn lock_machine(pool: &PgPool, id: i32, status: Option<&str>) -> Result<Machine> {
    update_machine_status(pool, id, true, status).await
}

pub async fn unlock_machine(pool: &PgPool, id: i32) -> Result<Machine> {
    update_machine_status(pool, id, false, None).await
}

pub async fn assign_snapshot(pool: &PgPool, id: i32, snapshot: String) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET snapshot = $1
        WHERE id = $2
        RETURNING
            id, name, label, arch, platform,
            ip, interface, tags, snapshot, locked, locked_changed_on, status,
            status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        "#,
    )
    .bind(snapshot)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "Failed to assign snapshot".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn update_machine_tags(pool: &PgPool, id: i32, tags: Vec<String>) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET tags = $1
        WHERE id = $2
        RETURNING
            id, name, label, arch, platform,
            ip, interface, tags, snapshot, locked, locked_changed_on, status,
            status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        "#,
    )
    .bind(tags)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "Failed to update machine tags".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn update_machine_network(
    pool: &PgPool,
    id: i32,
    ip: &str,
    interface: Option<&str>,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET
            ip = $1,
            interface = $2
        WHERE id = $3
        RETURNING
            id, name, label, arch, platform,
            ip, interface, tags, snapshot, locked, locked_changed_on, status,
            status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        "#,
    )
    .bind(ip)
    .bind(interface)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "Failed to update machine network".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn fetch_pool_machines(pool: &PgPool) -> Result<Vec<Machine>> {
    sqlx::query_as::<_, Machine>(
        r#"
        SELECT
            id, name, label, arch, platform, ip, interface, tags,
            snapshot, locked, locked_changed_on, status, status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        FROM "machines"
        WHERE pool_member = true
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| MachineError::FetchFailed { source: e }.into())
}

pub async fn upsert_pool_machine(
    pool: &PgPool,
    name: &str,
    label: &str,
    platform: MachinePlatform,
    arch: MachineArch,
    ip: &str,
    image_id: Uuid,
    provision_status: ProvisionStatusDb,
    clean_snapshot: Option<&str>,
    provider: &str,
    provider_id: &str,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        INSERT INTO "machines" (
            name, label, platform, arch, ip, locked, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        )
        VALUES ($1, $2, $3, $4, $5, false, false, $6, $7, $8, true, $9, $10, now())
        RETURNING
            id, name, label, arch, platform, ip, interface, tags,
            snapshot, locked, locked_changed_on, status, status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        "#,
    )
    .bind(name)
    .bind(label)
    .bind(platform)
    .bind(arch)
    .bind(ip)
    .bind(image_id)
    .bind(provision_status)
    .bind(clean_snapshot)
    .bind(provider)
    .bind(provider_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::InsertFailed {
            name: name.to_string(),
            message: "failed to upsert pool machine".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn update_provision_status(
    pool: &PgPool,
    id: i32,
    status: ProvisionStatusDb,
    clean_snapshot: Option<&str>,
) -> Result<Machine> {
    sqlx::query_as::<_, Machine>(
        r#"
        UPDATE "machines"
        SET provision_status = $1, clean_snapshot = $2, last_seen = now()
        WHERE id = $3
        RETURNING
            id, name, label, arch, platform, ip, interface, tags,
            snapshot, locked, locked_changed_on, status, status_changed_on, reserved,
            image_id, provision_status, clean_snapshot, pool_member,
            provider, provider_id, last_seen
        "#,
    )
    .bind(status)
    .bind(clean_snapshot)
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        MachineError::UpdateFailed {
            message: "Failed to update provision status".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn remove_pool_machine(pool: &PgPool, id: i32) -> Result<()> {
    sqlx::query(r#"UPDATE "machines" SET pool_member = false WHERE id = $1"#)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| MachineError::UpdateFailed {
            message: "Failed to remove from pool".to_string(),
            source: e,
        })?;
    Ok(())
}
