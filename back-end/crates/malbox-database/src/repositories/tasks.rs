use super::machinery::MachinePlatform;
use crate::error::{Result, TaskError};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, query_as};
use time::PrimitiveDateTime;

#[derive(sqlx::Type, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[sqlx(type_name = "task_state", rename_all = "snake_case")]
pub enum TaskState {
    Pending,
    Initializing,
    PreparingResources,
    Running,
    Stopping,
    Completed,
    Failed,
    Canceled,
}
#[derive(Debug, Clone, FromRow)]
pub struct Task {
    pub id: Option<i32>,
    pub target: String,
    pub plugins: Vec<String>,
    pub profile: Option<String>,
    pub platform: Option<MachinePlatform>,
    pub timeout: i64,
    pub enforce_timeout: Option<bool>,
    pub priority: i64,
    pub machine_id: Option<i32>,
    pub machine_memory: Option<i64>,
    pub machine_cpus: Option<i32>,
    pub created_on: PrimitiveDateTime,
    pub started_on: Option<PrimitiveDateTime>,
    pub completed_on: Option<PrimitiveDateTime>,
    pub status: TaskState,
    pub sample_id: Option<i64>,
    pub owner: Option<String>,
    pub tags: Option<Vec<String>>,
    pub snapshot_id: Option<uuid::Uuid>,
}

pub async fn insert_task(pool: &PgPool, task: Task) -> Result<Task> {
    query_as!(
        Task,
        r#"
        INSERT into "tasks" (
            target, plugins, profile, platform,
            timeout, enforce_timeout, priority, machine_id, machine_memory,
            machine_cpus, created_on, started_on, completed_on,
            status, sample_id, owner, tags, snapshot_id
        )
        VALUES (
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
        )
        RETURNING
            id, target, plugins, profile, platform AS "platform: MachinePlatform",
            timeout, enforce_timeout, priority, machine_id, machine_memory,
            machine_cpus, created_on, started_on, completed_on,
            status AS "status!: TaskState", sample_id, owner, tags, snapshot_id
        "#,
        task.target,
        &task.plugins,
        task.profile,
        task.platform as Option<MachinePlatform>,
        task.timeout,
        task.enforce_timeout,
        task.priority,
        task.machine_id,
        task.machine_memory,
        task.machine_cpus,
        task.created_on,
        task.started_on,
        task.completed_on,
        task.status as TaskState,
        task.sample_id,
        task.owner,
        task.tags.as_deref(),
        task.snapshot_id,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        TaskError::InsertFailed {
            name: task.target,
            message: "Failed to insert task".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn fetch_task(pool: &PgPool, id: i32) -> Result<Option<Task>> {
    query_as!(
        Task,
        r#"
        SELECT
            id, target, plugins, profile, platform AS "platform: MachinePlatform",
            timeout, enforce_timeout, priority, machine_id, machine_memory,
            machine_cpus, created_on, started_on, completed_on,
            status AS "status!: TaskState", sample_id, owner, tags, snapshot_id
        FROM "tasks" WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        TaskError::FetchFailed {
            message: "Failed to fetch task".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn fetch_all_tasks(pool: &PgPool) -> Result<Vec<Task>> {
    query_as!(
        Task,
        r#"
        SELECT
            id, target, plugins, profile, platform AS "platform: MachinePlatform",
            timeout, enforce_timeout, priority, machine_id, machine_memory,
            machine_cpus, created_on, started_on, completed_on,
            status AS "status!: TaskState", sample_id, owner, tags, snapshot_id
        FROM "tasks" ORDER BY created_on DESC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        TaskError::FetchFailed {
            message: "Failed to fetch all tasks".to_string(),
            source: e,
        }
        .into()
    })
}

pub async fn fetch_pending_tasks(pool: &PgPool) -> Result<Vec<Task>> {
    query_as!(
        Task,
        r#"
        SELECT
            id, target, plugins, profile, platform AS "platform: MachinePlatform",
            timeout, enforce_timeout, priority, machine_id, machine_memory,
            machine_cpus, created_on, started_on, completed_on,
            status AS "status!: TaskState", sample_id, owner, tags, snapshot_id
        FROM "tasks" WHERE status = 'pending'
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| {
        TaskError::FetchFailed {
            message: "Failed to fetch pending tasks".to_string(),
            source: e,
        }
        .into()
    })
}

/// Mark tasks stuck in transient states as Failed.
///
/// Called on scheduler startup to recover rows left in Running/Initializing/
/// PreparingResources/Stopping after a crash or kill — nothing else will ever
/// transition them, so they'd otherwise stay in those states forever.
/// Returns the number of rows reset.
pub async fn reset_orphaned_tasks(pool: &PgPool) -> Result<u64> {
    let utc_now = time::OffsetDateTime::now_utc();
    let now = PrimitiveDateTime::new(utc_now.date(), utc_now.time());

    let result = sqlx::query!(
        r#"
        UPDATE "tasks"
        SET status = 'failed', completed_on = $1
        WHERE status IN ('running', 'initializing', 'preparing_resources', 'stopping')
        "#,
        now,
    )
    .execute(pool)
    .await
    .map_err(|e| TaskError::FetchFailed {
        message: "Failed to reset orphaned tasks".to_string(),
        source: e,
    })?;

    Ok(result.rows_affected())
}

pub async fn update_task_status(pool: &PgPool, id: i32, status: TaskState) -> Result<Task> {
    query_as!(
        Task,
        r#"
        UPDATE "tasks"
        SET
            status = $1
        WHERE id = $2
        RETURNING
            id, target, plugins, profile, platform AS "platform: MachinePlatform",
            timeout, enforce_timeout, priority, machine_id, machine_memory,
            machine_cpus, created_on, started_on, completed_on,
            status AS "status!: TaskState", sample_id, owner, tags, snapshot_id
        "#,
        status as TaskState,
        id
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        TaskError::UpdateFailed {
            task_id: id,
            message: "Failed to update status".to_string(),
            source: e,
        }
        .into()
    })
}
