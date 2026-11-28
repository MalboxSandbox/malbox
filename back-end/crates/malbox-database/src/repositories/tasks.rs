//! `tasks`: analysis requests, their limits, lifecycle state, and final verdict.

use super::machinery::MachinePlatform;
use super::plugin_reports::DbClassification;
use crate::error::{Result, TaskError};
use serde::{Deserialize, Serialize};
use sqlx::{AssertSqlSafe, FromRow, PgConnection, PgPool, QueryBuilder};
use time::{OffsetDateTime, PrimitiveDateTime};
use uuid::Uuid;

/// Task lifecycle state (`task_state` Postgres enum).
#[derive(sqlx::Type, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[sqlx(type_name = "task_state", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
#[expect(
    clippy::exhaustive_enums,
    reason = "mirrors the `task_state` Postgres enum; matched exhaustively by malbox-scheduler"
)]
pub enum TaskState {
    /// Submitted, not yet started.
    Pending,
    /// Being analysed.
    Running,
    /// Finished successfully.
    Completed,
    /// Finished with an error.
    Failed,
    /// Stopped by a user.
    Canceled,
}

impl TaskState {
    /// Lowercase wire name, matching the `task_state` SQL enum.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    /// Whether the task has finished (completed, failed or canceled).
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
}

/// Every column of `tasks`, in table order. Shared by all task queries.
const TASK_COLUMNS: &str = "id, target, plugins, excluded_plugins, timeout, priority, machine_id, \
    platform, snapshot_id, params, max_depth, max_artifacts, max_detonations, max_total_bytes, \
    created_on, started_on, completed_on, updated_at, deadline, status, sample_id, parent_task_id, \
    owner, tags, error_count, artifact_count, partial, shared_data, verdict_classification, \
    verdict_score, verdict_artifact_id";

/// A row of the `tasks` table.
#[derive(Debug, Clone, FromRow)]
pub struct Task {
    /// Database primary key.
    pub id: i32,
    /// What was submitted for analysis, e.g. the sample's file name.
    pub target: String,
    /// Plugins explicitly requested; empty means the defaults.
    pub plugins: Vec<String>,
    /// Plugins excluded from the run.
    pub excluded_plugins: Vec<String>,
    /// Per-detonation timeout in seconds.
    pub timeout: i64,
    /// Scheduling priority; higher runs first.
    pub priority: i64,
    /// Machine the task must run on.
    pub machine_id: Option<i32>,
    /// Guest platform the task must run on.
    pub platform: Option<MachinePlatform>,
    /// Snapshot the task must run on.
    pub snapshot_id: Option<Uuid>,
    /// Free-form analysis parameters as JSON.
    pub params: serde_json::Value,
    /// Maximum artifact DAG depth.
    pub max_depth: i32,
    /// Maximum number of artifacts.
    pub max_artifacts: i32,
    /// Maximum number of detonations.
    pub max_detonations: i32,
    /// Maximum total bytes of stored artifact blobs.
    pub max_total_bytes: i64,
    /// When the task was submitted (UTC).
    pub created_on: PrimitiveDateTime,
    /// When the task first started running (UTC).
    pub started_on: Option<PrimitiveDateTime>,
    /// When the task reached a terminal state (UTC).
    pub completed_on: Option<PrimitiveDateTime>,
    /// When the row was last modified.
    pub updated_at: Option<PrimitiveDateTime>,
    /// Time after which unfinished work is abandoned.
    pub deadline: OffsetDateTime,
    /// Lifecycle state.
    pub status: TaskState,
    /// Sample analysed by the task.
    pub sample_id: Option<i64>,
    /// Task this one was promoted from.
    pub parent_task_id: Option<i32>,
    /// User who submitted the task.
    pub owner: Option<String>,
    /// User-assigned tags.
    pub tags: Option<Vec<String>>,
    /// Number of jobs that failed.
    pub error_count: i32,
    /// Number of artifacts in the task's DAG.
    pub artifact_count: i32,
    /// Whether some analysis was skipped or failed, so results are incomplete.
    pub partial: bool,
    /// JSON data plugins share across the task's jobs.
    pub shared_data: serde_json::Value,
    /// Final worst-wins classification.
    pub verdict_classification: Option<DbClassification>,
    /// Final threat score.
    pub verdict_score: Option<i16>,
    /// Artifact that determined the verdict.
    pub verdict_artifact_id: Option<i64>,
}

/// Insertable columns of `tasks`. `created_on` is set to the current UTC
/// time and `status` to `pending`. See [`Task`] for field meanings.
#[derive(Debug, Clone)]
pub struct NewTask<'a> {
    /// What was submitted for analysis.
    pub target: &'a str,
    /// Plugins explicitly requested.
    pub plugins: &'a [String],
    /// Plugins excluded from the run.
    pub excluded_plugins: &'a [String],
    /// Per-detonation timeout in seconds.
    pub timeout: i64,
    /// Scheduling priority.
    pub priority: i64,
    /// Machine the task must run on.
    pub machine_id: Option<i32>,
    /// Guest platform the task must run on.
    pub platform: Option<MachinePlatform>,
    /// Snapshot the task must run on.
    pub snapshot_id: Option<Uuid>,
    /// Free-form analysis parameters.
    pub params: &'a serde_json::Value,
    /// Maximum artifact DAG depth.
    pub max_depth: i32,
    /// Maximum number of artifacts.
    pub max_artifacts: i32,
    /// Maximum number of detonations.
    pub max_detonations: i32,
    /// Maximum total bytes of stored artifact blobs.
    pub max_total_bytes: i64,
    /// Time after which unfinished work is abandoned.
    pub deadline: OffsetDateTime,
    /// Sample analysed by the task.
    pub sample_id: Option<i64>,
    /// Task this one was promoted from.
    pub parent_task_id: Option<i32>,
    /// Submitting user.
    pub owner: Option<&'a str>,
    /// User-assigned tags.
    pub tags: Option<&'a [String]>,
    /// Initial plugin-shared JSON data.
    pub shared_data: &'a serde_json::Value,
}

/// Final bookkeeping written when a task reaches a terminal state.
#[derive(Debug, Clone)]
pub struct TaskFinalization {
    /// Terminal state to record.
    pub status: TaskState,
    /// Number of jobs that failed.
    pub error_count: i32,
    /// Number of artifacts in the task's DAG.
    pub artifact_count: i32,
    /// Whether results are incomplete.
    pub partial: bool,
    /// Final worst-wins classification.
    pub verdict_classification: Option<DbClassification>,
    /// Final threat score.
    pub verdict_score: Option<i16>,
    /// Artifact that determined the verdict.
    pub verdict_artifact_id: Option<i64>,
}

fn fetch_err(message: &str, source: sqlx::Error) -> crate::error::DatabaseError {
    TaskError::FetchFailed {
        message: message.to_string(),
        source,
    }
    .into()
}

fn update_err(task_id: i32, message: &str, source: sqlx::Error) -> crate::error::DatabaseError {
    TaskError::UpdateFailed {
        task_id,
        message: message.to_string(),
        source,
    }
    .into()
}

/// Insert a task. Runs on a caller-supplied connection so the root artifact
/// can be inserted in the same transaction.
///
/// # Errors
///
/// Returns [`TaskError::InsertFailed`] when the insert fails.
pub async fn insert_task(conn: &mut PgConnection, t: &NewTask<'_>) -> Result<Task> {
    let sql = format!(
        "insert into tasks (target, plugins, excluded_plugins, timeout, priority, machine_id, \
         platform, snapshot_id, params, max_depth, max_artifacts, max_detonations, max_total_bytes, \
         created_on, deadline, status, sample_id, parent_task_id, owner, tags, shared_data) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
         (now() at time zone 'utc'), $14, 'pending', $15, $16, $17, $18, $19) \
         returning {TASK_COLUMNS}"
    );
    sqlx::query_as::<_, Task>(AssertSqlSafe(sql))
        .bind(t.target)
        .bind(t.plugins)
        .bind(t.excluded_plugins)
        .bind(t.timeout)
        .bind(t.priority)
        .bind(t.machine_id)
        .bind(t.platform.clone())
        .bind(t.snapshot_id)
        .bind(t.params)
        .bind(t.max_depth)
        .bind(t.max_artifacts)
        .bind(t.max_detonations)
        .bind(t.max_total_bytes)
        .bind(t.deadline)
        .bind(t.sample_id)
        .bind(t.parent_task_id)
        .bind(t.owner)
        .bind(t.tags)
        .bind(t.shared_data)
        .fetch_one(conn)
        .await
        .map_err(|e| {
            TaskError::InsertFailed {
                name: t.target.to_string(),
                message: "Failed to insert task".to_string(),
                source: e,
            }
            .into()
        })
}

/// Fetch one task by id, `None` when it does not exist.
///
/// # Errors
///
/// Returns [`TaskError::FetchFailed`] when the query fails.
pub async fn fetch_task(pool: &PgPool, id: i32) -> Result<Option<Task>> {
    let sql = format!("select {TASK_COLUMNS} from tasks where id = $1");
    sqlx::query_as::<_, Task>(AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| fetch_err("Failed to fetch task", e))
}

/// Ids of tasks the orchestrator still has to drive.
///
/// # Errors
///
/// Returns [`TaskError::FetchFailed`] when the query fails.
pub async fn fetch_active_task_ids(pool: &PgPool) -> Result<Vec<i32>> {
    sqlx::query_scalar::<_, i32>(
        "select id from tasks where status in ('pending', 'running') order by id",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| fetch_err("Failed to fetch active tasks", e))
}

/// Mark a task `running`, stamping `started_on` the first time.
///
/// # Errors
///
/// Returns [`TaskError::UpdateFailed`] when the update fails.
pub async fn set_task_running(pool: &PgPool, id: i32) -> Result<()> {
    sqlx::query(
        "update tasks set status = 'running', \
         started_on = coalesce(started_on, (now() at time zone 'utc')) where id = $1",
    )
    .bind(id)
    .execute(pool)
    .await
    .map_err(|e| update_err(id, "Failed to mark task running", e))?;
    Ok(())
}

/// Write a task's terminal state and final bookkeeping, stamping `completed_on`.
///
/// # Errors
///
/// Returns [`TaskError::UpdateFailed`] when the update fails.
pub async fn finalize_task(pool: &PgPool, id: i32, f: &TaskFinalization) -> Result<()> {
    sqlx::query(
        "update tasks set status = $2, error_count = $3, artifact_count = $4, partial = $5, \
         verdict_classification = $6, verdict_score = $7, verdict_artifact_id = $8, \
         completed_on = (now() at time zone 'utc') where id = $1",
    )
    .bind(id)
    .bind(f.status)
    .bind(f.error_count)
    .bind(f.artifact_count)
    .bind(f.partial)
    .bind(f.verdict_classification)
    .bind(f.verdict_score)
    .bind(f.verdict_artifact_id)
    .execute(pool)
    .await
    .map_err(|e| update_err(id, "Failed to finalize task", e))?;
    Ok(())
}

/// Return a terminal task to `running` with a fresh `deadline` (rescan of a
/// finished task); the old deadline has usually passed already.
///
/// # Errors
///
/// Returns [`TaskError::UpdateFailed`] when the update fails.
pub async fn reopen_task(pool: &PgPool, id: i32, deadline: OffsetDateTime) -> Result<()> {
    sqlx::query(
        "update tasks set status = 'running', completed_on = null, deadline = $2 where id = $1",
    )
    .bind(id)
    .bind(deadline)
    .execute(pool)
    .await
    .map_err(|e| update_err(id, "Failed to reopen task", e))?;
    Ok(())
}

/// Filter and pagination parameters for task listing.
#[derive(Debug, Clone)]
pub struct TaskFilter {
    /// Only tasks in one of these states.
    pub statuses: Option<Vec<TaskState>>,
    /// Only tasks for this guest platform.
    pub platform: Option<MachinePlatform>,
    /// Only tasks submitted by this user.
    pub owner: Option<String>,
    /// Only tasks carrying this tag.
    pub tag: Option<String>,
    /// Only tasks submitted at or after this time.
    pub after: Option<PrimitiveDateTime>,
    /// Only tasks submitted at or before this time.
    pub before: Option<PrimitiveDateTime>,
    /// Keyset cursor: `created_on` of the last task on the previous page.
    pub cursor_created_on: Option<PrimitiveDateTime>,
    /// Keyset cursor: id of the last task on the previous page.
    pub cursor_id: Option<i32>,
    /// Requested page size, clamped to 1..=200.
    pub limit: i64,
}

/// One page of tasks, newest first.
#[derive(Debug, Clone)]
pub struct TaskPage {
    /// Tasks on this page.
    pub items: Vec<Task>,
    /// Whether more tasks follow this page.
    pub has_more: bool,
}

/// Largest page [`fetch_tasks_page`] returns.
const MAX_PAGE_LEN: u8 = 200;

fn push_filters(qb: &mut QueryBuilder<sqlx::Postgres>, filter: &TaskFilter) {
    if let Some(statuses) = &filter.statuses {
        qb.push(" AND status = ANY(");
        qb.push_bind(statuses.clone());
        qb.push(")");
    }
    if let Some(platform) = &filter.platform {
        qb.push(" AND platform = ");
        qb.push_bind(platform.clone());
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
}

/// One page of tasks matching `filter`, ordered by `created_on` then id, descending.
///
/// # Errors
///
/// Returns [`TaskError::FetchFailed`] when the query fails.
pub async fn fetch_tasks_page(pool: &PgPool, filter: &TaskFilter) -> Result<TaskPage> {
    let limit = filter.limit.clamp(1, i64::from(MAX_PAGE_LEN));
    let fetch_limit = limit + 1;
    // Same clamp in `usize`; a negative request falls back to the minimum of 1.
    let page_len =
        usize::try_from(filter.limit).map_or(1, |l| l.clamp(1, usize::from(MAX_PAGE_LEN)));

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new(format!("SELECT {TASK_COLUMNS} FROM tasks WHERE TRUE"));
    push_filters(&mut qb, filter);

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
        .map_err(|e| fetch_err("Failed to fetch tasks page", e))?;

    let has_more = rows.len() > page_len;
    rows.truncate(page_len);

    Ok(TaskPage {
        items: rows,
        has_more,
    })
}

/// Number of tasks matching a filter, in total and per state.
#[derive(Debug, Clone)]
pub struct TaskCounts {
    /// Total matching tasks.
    pub total: i64,
    /// Matching tasks per `task_state` label.
    pub by_status: Vec<(String, i64)>,
}

/// Count tasks matching `filter`; pagination fields are ignored.
///
/// # Errors
///
/// Returns [`TaskError::FetchFailed`] when the query fails.
pub async fn count_tasks(pool: &PgPool, filter: &TaskFilter) -> Result<TaskCounts> {
    #[derive(FromRow)]
    struct StatusCount {
        status: String,
        count: i64,
    }

    let mut qb: QueryBuilder<sqlx::Postgres> =
        QueryBuilder::new("SELECT status::text AS status, COUNT(*) AS count FROM tasks WHERE TRUE");
    push_filters(&mut qb, filter);
    qb.push(" GROUP BY status");

    let rows: Vec<StatusCount> = qb
        .build_query_as::<StatusCount>()
        .fetch_all(pool)
        .await
        .map_err(|e| fetch_err("Failed to count tasks", e))?;

    let total = rows.iter().map(|r| r.count).sum();
    let by_status = rows.into_iter().map(|r| (r.status, r.count)).collect();

    Ok(TaskCounts { total, by_status })
}

/// Ids of the tasks that analysed a sample, newest first.
///
/// # Errors
///
/// Returns [`TaskError::FetchFailed`] when the query fails.
pub async fn fetch_tasks_by_sample_id(pool: &PgPool, sample_id: i64) -> Result<Vec<i32>> {
    sqlx::query_scalar::<_, i32>(
        "select id from tasks where sample_id = $1 order by created_on desc",
    )
    .bind(sample_id)
    .fetch_all(pool)
    .await
    .map_err(|e| fetch_err("Failed to fetch tasks by sample", e))
}

/// Tasks that analysed a sample, newest first.
///
/// # Errors
///
/// Returns [`TaskError::FetchFailed`] when the query fails.
pub async fn fetch_tasks_for_sample(pool: &PgPool, sample_id: i64) -> Result<Vec<Task>> {
    let sql =
        format!("select {TASK_COLUMNS} from tasks where sample_id = $1 order by created_on desc");
    sqlx::query_as::<_, Task>(AssertSqlSafe(sql))
        .bind(sample_id)
        .fetch_all(pool)
        .await
        .map_err(|e| fetch_err("Failed to fetch tasks for sample", e))
}
