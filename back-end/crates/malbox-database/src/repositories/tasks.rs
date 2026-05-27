use super::machinery::MachinePlatform;
use crate::error::{Result, TaskError};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, QueryBuilder, query_as};
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

/// Filter and pagination parameters for task listing.
pub struct TaskFilter {
    pub statuses: Option<Vec<TaskState>>,
    pub platform: Option<MachinePlatform>,
    pub owner: Option<String>,
    pub tag: Option<String>,
    pub after: Option<PrimitiveDateTime>,
    pub before: Option<PrimitiveDateTime>,
    pub cursor_created_on: Option<PrimitiveDateTime>,
    pub cursor_id: Option<i32>,
    pub limit: i64,
}

pub struct TaskPage {
    pub items: Vec<Task>,
    pub has_more: bool,
}

pub async fn fetch_tasks_page(pool: &PgPool, filter: &TaskFilter) -> Result<TaskPage> {
    let limit = filter.limit.clamp(1, 200);
    let fetch_limit = limit + 1;

    let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new(
        "SELECT id, target, plugins, profile, platform, \
         timeout, enforce_timeout, priority, machine_id, machine_memory, \
         machine_cpus, created_on, started_on, completed_on, \
         status, sample_id, owner, tags, snapshot_id \
         FROM tasks WHERE TRUE",
    );

    if let Some(statuses) = &filter.statuses {
        let literals: Vec<&str> = statuses.iter().map(task_state_sql).collect();
        qb.push(" AND status = ANY(ARRAY[");
        for (i, lit) in literals.iter().enumerate() {
            if i > 0 {
                qb.push(",");
            }
            qb.push(format!("'{}'::task_state", lit));
        }
        qb.push("])");
    }

    if let Some(platform) = &filter.platform {
        let lit = match platform {
            MachinePlatform::Windows => "windows",
            MachinePlatform::Linux => "linux",
        };
        qb.push(format!(" AND platform = '{}'::machine_platform", lit));
    }

    if let Some(owner) = &filter.owner {
        qb.push(" AND owner = ");
        qb.push_bind(owner.clone());
    }

    if let Some(tag) = &filter.tag {
        qb.push(" AND ");
        qb.push_bind(tag.clone());
        qb.push(" = ANY(tags)");
    }

    if let Some(after) = &filter.after {
        qb.push(" AND created_on >= ");
        qb.push_bind(*after);
    }

    if let Some(before) = &filter.before {
        qb.push(" AND created_on <= ");
        qb.push_bind(*before);
    }

    if let (Some(cursor_ts), Some(cursor_id)) = (&filter.cursor_created_on, &filter.cursor_id) {
        qb.push(" AND (created_on, id) < (");
        qb.push_bind(*cursor_ts);
        qb.push(", ");
        qb.push_bind(*cursor_id);
        qb.push(")");
    }

    qb.push(" ORDER BY created_on DESC, id DESC LIMIT ");
    qb.push_bind(fetch_limit);

    let mut rows: Vec<Task> = qb
        .build_query_as::<Task>()
        .fetch_all(pool)
        .await
        .map_err(|e| TaskError::FetchFailed {
            message: "Failed to fetch tasks page".to_string(),
            source: e,
        })?;

    let has_more = rows.len() as i64 > limit;
    rows.truncate(limit as usize);

    Ok(TaskPage {
        items: rows,
        has_more,
    })
}

pub struct TaskCounts {
    pub total: i64,
    pub by_status: Vec<(String, i64)>,
}

pub async fn count_tasks(pool: &PgPool, filter: &TaskFilter) -> Result<TaskCounts> {
    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT status::text, COUNT(*) as count FROM tasks WHERE TRUE");

    if let Some(statuses) = &filter.statuses {
        let literals: Vec<&str> = statuses.iter().map(task_state_sql).collect();
        qb.push(" AND status = ANY(ARRAY[");
        for (i, lit) in literals.iter().enumerate() {
            if i > 0 {
                qb.push(",");
            }
            qb.push(format!("'{}'::task_state", lit));
        }
        qb.push("])");
    }

    if let Some(platform) = &filter.platform {
        let lit = match platform {
            MachinePlatform::Windows => "windows",
            MachinePlatform::Linux => "linux",
        };
        qb.push(format!(" AND platform = '{}'::machine_platform", lit));
    }

    if let Some(owner) = &filter.owner {
        qb.push(" AND owner = ");
        qb.push_bind(owner.clone());
    }

    if let Some(tag) = &filter.tag {
        qb.push(" AND ");
        qb.push_bind(tag.clone());
        qb.push(" = ANY(tags)");
    }

    if let Some(after) = &filter.after {
        qb.push(" AND created_on >= ");
        qb.push_bind(*after);
    }

    if let Some(before) = &filter.before {
        qb.push(" AND created_on <= ");
        qb.push_bind(*before);
    }

    qb.push(" GROUP BY status");

    #[derive(FromRow)]
    struct StatusCount {
        status: String,
        count: i64,
    }

    let rows: Vec<StatusCount> = qb
        .build_query_as::<StatusCount>()
        .fetch_all(pool)
        .await
        .map_err(|e| TaskError::FetchFailed {
            message: "Failed to count tasks".to_string(),
            source: e,
        })?;

    let total = rows.iter().map(|r| r.count).sum();
    let by_status = rows.into_iter().map(|r| (r.status, r.count)).collect();

    Ok(TaskCounts { total, by_status })
}

pub async fn fetch_tasks_by_sample_id(pool: &PgPool, sample_id: i64) -> Result<Vec<i32>> {
    struct TaskId {
        id: Option<i32>,
    }

    let rows = sqlx::query_as!(
        TaskId,
        r#"SELECT id FROM tasks WHERE sample_id = $1 ORDER BY created_on DESC"#,
        sample_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| TaskError::FetchFailed {
        message: "Failed to fetch tasks by sample".to_string(),
        source: e,
    })?;

    Ok(rows.into_iter().filter_map(|r| r.id).collect())
}

fn task_state_sql(state: &TaskState) -> &'static str {
    match state {
        TaskState::Pending => "pending",
        TaskState::Initializing => "initializing",
        TaskState::PreparingResources => "preparing_resources",
        TaskState::Running => "running",
        TaskState::Stopping => "stopping",
        TaskState::Completed => "completed",
        TaskState::Failed => "failed",
        TaskState::Canceled => "canceled",
    }
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
