//! `jobs`: one row per (artifact, analyzer).
//!
//! Enum columns (`kind`, `stage`, `state`, `error_class`) travel as text,
//! using the shared literal strings.

use std::fmt;

use crate::error::{DatabaseError, JobError, Result};
use serde::Serialize;
use sqlx::{AssertSqlSafe, FromRow, PgExecutor, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

/// Every column of `jobs`, enum columns cast to text.
const JOB_COLUMNS: &str = "id, task_id, artifact_id, kind::text as kind, analyzer, \
    stage::text as stage, priority, state::text as state, attempts, max_attempts, reruns, \
    ignore_cache, error_class::text as error_class, error_message, cache_key, cached_from_job_id, \
    drop_requested, machine_id, snapshot_id, guest_plugins, detonation, not_before, started_on, \
    completed_on, created_on";

/// Terminal job states.
pub const TERMINAL_STATES: [&str; 5] = ["completed", "failed", "skipped", "cached", "canceled"];

/// One analyzer run over one artifact.
#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Job {
    /// Database primary key.
    pub id: i64,
    /// Task the job belongs to.
    pub task_id: i32,
    /// Artifact the job analyses.
    pub artifact_id: i64,
    /// Where the job runs (`job_kind` as text, e.g. `"host"`, `"detonation"`).
    pub kind: String,
    /// Name of the analyzer plugin.
    pub analyzer: String,
    /// Analysis stage the job belongs to (`analysis_stage` as text).
    pub stage: String,
    /// Scheduling priority; higher runs first.
    pub priority: i64,
    /// Lifecycle state (`job_state` as text); see [`TERMINAL_STATES`].
    pub state: String,
    /// Attempts started so far.
    pub attempts: i16,
    /// Attempts allowed before the job fails for good.
    pub max_attempts: i16,
    /// Times the job was reset for a manual rerun.
    pub reruns: i16,
    /// Whether the result cache is bypassed for this job.
    pub ignore_cache: bool,
    /// Failure category of the last attempt (`job_error_class` as text).
    pub error_class: Option<String>,
    /// Human-readable failure message of the last attempt.
    pub error_message: Option<String>,
    /// Result-cache key computed for the job, when cacheable.
    pub cache_key: Option<String>,
    /// Job whose cached result this job reused.
    pub cached_from_job_id: Option<i64>,
    /// Whether cancellation of the running job was requested.
    pub drop_requested: bool,
    /// Machine assigned to a detonation job.
    pub machine_id: Option<i32>,
    /// Snapshot the detonation machine was reverted to.
    pub snapshot_id: Option<Uuid>,
    /// Guest plugins requested for, or run by, a detonation.
    pub guest_plugins: Option<Vec<String>>,
    /// Detonation parameters as JSON.
    pub detonation: Option<serde_json::Value>,
    /// Earliest time the job may be claimed again after a failed attempt.
    pub not_before: Option<OffsetDateTime>,
    /// When the current attempt started.
    pub started_on: Option<OffsetDateTime>,
    /// When the job reached a terminal state.
    pub completed_on: Option<OffsetDateTime>,
    /// When the row was inserted.
    pub created_on: OffsetDateTime,
}

impl Job {
    /// Whether the job's state is one of [`TERMINAL_STATES`].
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        TERMINAL_STATES.contains(&self.state.as_str())
    }
}

/// Column values for inserting a [`Job`]; see its fields for meanings.
#[derive(Debug, Clone)]
pub struct NewJob<'a> {
    /// Owning task.
    pub task_id: i32,
    /// Artifact to analyse.
    pub artifact_id: i64,
    /// `job_kind` literal.
    pub kind: &'a str,
    /// Name of the analyzer plugin.
    pub analyzer: &'a str,
    /// `analysis_stage` literal.
    pub stage: &'a str,
    /// Scheduling priority; higher runs first.
    pub priority: i64,
    /// Initial `job_state` literal.
    pub state: &'a str,
    /// Attempts allowed before the job fails for good.
    pub max_attempts: i16,
    /// Whether the result cache is bypassed.
    pub ignore_cache: bool,
    /// Result-cache key, when cacheable.
    pub cache_key: Option<&'a str>,
    /// Snapshot a detonation must run on.
    pub snapshot_id: Option<Uuid>,
    /// Guest plugins requested for a detonation.
    pub guest_plugins: Option<&'a [String]>,
    /// Detonation parameters as JSON.
    pub detonation: Option<serde_json::Value>,
}

fn insert_err(target: fmt::Arguments<'_>, source: sqlx::Error) -> DatabaseError {
    JobError::InsertFailed {
        target: target.to_string(),
        source,
    }
    .into()
}

fn fetch_err(target: fmt::Arguments<'_>, source: sqlx::Error) -> DatabaseError {
    JobError::FetchFailed {
        target: target.to_string(),
        source,
    }
    .into()
}

fn update_err(target: fmt::Arguments<'_>, source: sqlx::Error) -> DatabaseError {
    JobError::UpdateFailed {
        target: target.to_string(),
        source,
    }
    .into()
}

/// Insert a job and return the stored row.
///
/// # Errors
///
/// Returns [`JobError::InsertFailed`] when the insert fails.
pub async fn insert_job(conn: impl PgExecutor<'_>, j: &NewJob<'_>) -> Result<Job> {
    let sql = format!(
        "insert into jobs (task_id, artifact_id, kind, analyzer, stage, priority, state, \
         max_attempts, ignore_cache, cache_key, snapshot_id, guest_plugins, detonation) \
         values ($1, $2, $3::job_kind, $4, $5::analysis_stage, $6, $7::job_state, $8, $9, $10, \
         $11, $12, $13) returning {JOB_COLUMNS}"
    );
    sqlx::query_as::<_, Job>(AssertSqlSafe(sql))
        .bind(j.task_id)
        .bind(j.artifact_id)
        .bind(j.kind)
        .bind(j.analyzer)
        .bind(j.stage)
        .bind(j.priority)
        .bind(j.state)
        .bind(j.max_attempts)
        .bind(j.ignore_cache)
        .bind(j.cache_key)
        .bind(j.snapshot_id)
        .bind(j.guest_plugins)
        .bind(&j.detonation)
        .fetch_one(conn)
        .await
        .map_err(|e| insert_err(format_args!("job for artifact {}", j.artifact_id), e))
}

/// Every job of a task, in insertion order.
///
/// # Errors
///
/// Returns [`JobError::FetchFailed`] when the query fails.
pub async fn fetch_task_jobs(pool: &PgPool, task_id: i32) -> Result<Vec<Job>> {
    let sql = format!("select {JOB_COLUMNS} from jobs where task_id = $1 order by id");
    sqlx::query_as::<_, Job>(AssertSqlSafe(sql))
        .bind(task_id)
        .fetch_all(pool)
        .await
        .map_err(|e| fetch_err(format_args!("jobs of task {task_id}"), e))
}

/// Fetch one job by id, `None` when it does not exist.
///
/// # Errors
///
/// Returns [`JobError::FetchFailed`] when the query fails.
pub async fn fetch_job(pool: &PgPool, id: i64) -> Result<Option<Job>> {
    let sql = format!("select {JOB_COLUMNS} from jobs where id = $1");
    sqlx::query_as::<_, Job>(AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| fetch_err(format_args!("job {id}"), e))
}

/// Set a job's state and error fields; terminal states also stamp
/// `completed_on`.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn set_job_state(
    conn: impl PgExecutor<'_>,
    id: i64,
    state: &str,
    error_class: Option<&str>,
    error_message: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "update jobs set state = $2::job_state, error_class = $3::job_error_class, \
         error_message = $4, \
         completed_on = case when $2 = any($5) then now() else completed_on end \
         where id = $1",
    )
    .bind(id)
    .bind(state)
    .bind(error_class)
    .bind(error_message)
    .bind(&TERMINAL_STATES[..])
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}

/// Move `blocked` jobs to `ready`. Returns the number released.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn release_jobs(conn: impl PgExecutor<'_>, ids: &[i64]) -> Result<u64> {
    let r = sqlx::query("update jobs set state = 'ready' where id = any($1) and state = 'blocked'")
        .bind(ids)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("jobs {ids:?}"), e))?;
    Ok(r.rows_affected())
}

/// Requeue a job for a manual rerun, clearing its error and timing fields
/// and incrementing `reruns`.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "`ignore_cache` is the value written to the boolean column, not a mode switch"
)]
pub async fn reset_job_for_rerun(
    conn: impl PgExecutor<'_>,
    id: i64,
    ignore_cache: bool,
) -> Result<()> {
    sqlx::query(
        "update jobs set state = 'ready', reruns = reruns + 1, ignore_cache = $2, \
         error_class = null, error_message = null, started_on = null, completed_on = null, \
         drop_requested = false, \
         not_before = null where id = $1",
    )
    .bind(id)
    .bind(ignore_cache)
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}

/// Reset a finished job for a rescan. It goes straight to `blocked`, never
/// through `ready`, so no executor can claim it before progression releases
/// it stage by stage (with cache lookup).
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
#[expect(
    clippy::fn_params_excessive_bools,
    reason = "`ignore_cache` is the value written to the boolean column, not a mode switch"
)]
pub async fn reset_job_for_rescan(
    conn: impl PgExecutor<'_>,
    id: i64,
    ignore_cache: bool,
) -> Result<()> {
    sqlx::query(
        "update jobs set state = 'blocked', attempts = 0, ignore_cache = $2, \
         error_class = null, error_message = null, started_on = null, completed_on = null, \
         drop_requested = false, \
         not_before = null where id = $1",
    )
    .bind(id)
    .bind(ignore_cache)
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}

/// Put a failed attempt back in the queue, not before `not_before`.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn schedule_retry(
    conn: impl PgExecutor<'_>,
    id: i64,
    error_class: &str,
    error_message: &str,
    not_before: OffsetDateTime,
) -> Result<()> {
    sqlx::query(
        "update jobs set state = 'ready', error_class = $2::job_error_class, error_message = $3, \
         not_before = $4, started_on = null where id = $1",
    )
    .bind(id)
    .bind(error_class)
    .bind(error_message)
    .bind(not_before)
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}

/// Finish a job by reusing the cached result of `from_job_id`.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn mark_cached(conn: impl PgExecutor<'_>, id: i64, from_job_id: i64) -> Result<()> {
    sqlx::query(
        "update jobs set state = 'cached', cached_from_job_id = $2, completed_on = now() \
         where id = $1",
    )
    .bind(id)
    .bind(from_job_id)
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}

/// Ask the executor running the job to cancel it.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn set_drop_requested(conn: impl PgExecutor<'_>, id: i64) -> Result<()> {
    sqlx::query("update jobs set drop_requested = true where id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}

/// Re-prioritise an artifact's unstarted jobs.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn set_job_priority_for_artifact(
    conn: impl PgExecutor<'_>,
    artifact_id: i64,
    priority: i64,
) -> Result<()> {
    sqlx::query(
        "update jobs set priority = $2 where artifact_id = $1 and state in ('blocked', 'ready')",
    )
    .bind(artifact_id)
    .bind(priority)
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("jobs of artifact {artifact_id}"), e))?;
    Ok(())
}

/// Claim the highest-priority ready host job whose analyzer is not busy and
/// whose task is below its running-job limit.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the claiming update fails.
pub async fn claim_host_job(
    pool: &PgPool,
    busy_plugins: &[String],
    max_running_per_task: i64,
) -> Result<Option<Job>> {
    let sql = format!(
        "update jobs set state = 'running', attempts = attempts + 1, started_on = now() \
         where id = (select j.id from jobs j \
           where j.state = 'ready' and j.kind = 'host' \
             and (j.not_before is null or j.not_before <= now()) \
             and j.analyzer <> all($1) \
             and (select count(*) from jobs r where r.task_id = j.task_id and r.state = 'running') < $2 \
           order by j.priority desc, j.id limit 1 for update skip locked) \
         returning {JOB_COLUMNS}"
    );
    sqlx::query_as::<_, Job>(AssertSqlSafe(sql))
        .bind(busy_plugins)
        .bind(max_running_per_task)
        .fetch_optional(pool)
        .await
        .map_err(|e| update_err(format_args!("next ready host job"), e))
}

/// Ready detonation jobs whose retry delay has passed, highest priority first.
///
/// # Errors
///
/// Returns [`JobError::FetchFailed`] when the query fails.
pub async fn fetch_ready_detonation_jobs(pool: &PgPool, limit: i64) -> Result<Vec<Job>> {
    let sql = format!(
        "select {JOB_COLUMNS} from jobs where state = 'ready' and kind = 'detonation' \
         and (not_before is null or not_before <= now()) \
         order by priority desc, id limit $1"
    );
    sqlx::query_as::<_, Job>(AssertSqlSafe(sql))
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(|e| fetch_err(format_args!("ready detonation jobs"), e))
}

/// Claim a specific job if it is still `ready`.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the claiming update fails.
pub async fn claim_job_by_id(conn: impl PgExecutor<'_>, id: i64) -> Result<Option<Job>> {
    let sql = format!(
        "update jobs set state = 'running', attempts = attempts + 1, started_on = now() \
         where id = $1 and state = 'ready' returning {JOB_COLUMNS}"
    );
    sqlx::query_as::<_, Job>(AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(conn)
        .await
        .map_err(|e| update_err(format_args!("job {id}"), e))
}

/// Startup recovery: jobs left `running` by a previous daemon either fail
/// (attempts exhausted) or go back to `ready`. Returns the rows touched.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when either update fails.
pub async fn recover_running_jobs(pool: &PgPool) -> Result<u64> {
    let failed = sqlx::query(
        "update jobs set state = 'failed', error_class = 'recoverable', \
         error_message = 'interrupted by daemon restart', completed_on = now() \
         where state = 'running' and attempts >= max_attempts",
    )
    .execute(pool)
    .await
    .map_err(|e| update_err(format_args!("running jobs"), e))?;
    let requeued = sqlx::query(
        "update jobs set state = 'ready', not_before = null, started_on = null \
         where state = 'running'",
    )
    .execute(pool)
    .await
    .map_err(|e| update_err(format_args!("running jobs"), e))?;
    Ok(failed.rows_affected() + requeued.rows_affected())
}

/// Number of a task's jobs currently `running`.
///
/// # Errors
///
/// Returns [`JobError::FetchFailed`] when the query fails.
pub async fn count_running_jobs(pool: &PgPool, task_id: i32) -> Result<i64> {
    sqlx::query_scalar::<_, i64>(
        "select count(*) from jobs where task_id = $1 and state = 'running'",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|e| fetch_err(format_args!("running jobs of task {task_id}"), e))
}

/// Cancel the given jobs that have not started. Returns the ids canceled;
/// the others were claimed concurrently.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn cancel_unstarted_jobs(conn: impl PgExecutor<'_>, ids: &[i64]) -> Result<Vec<i64>> {
    sqlx::query_scalar::<_, i64>(
        "update jobs set state = 'canceled', completed_on = now() \
         where id = any($1) and state in ('blocked', 'ready') returning id",
    )
    .bind(ids)
    .fetch_all(conn)
    .await
    .map_err(|e| update_err(format_args!("jobs {ids:?}"), e))
}

/// Record the machine and snapshot a detonation job was assigned.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn set_job_machine(
    conn: impl PgExecutor<'_>,
    id: i64,
    machine_id: i32,
    snapshot_id: Uuid,
) -> Result<()> {
    sqlx::query("update jobs set machine_id = $2, snapshot_id = $3 where id = $1")
        .bind(id)
        .bind(machine_id)
        .bind(snapshot_id)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}

/// Record the guest plugins a detonation actually ran.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn set_job_guest_plugins(
    conn: impl PgExecutor<'_>,
    id: i64,
    names: &[String],
) -> Result<()> {
    sqlx::query("update jobs set guest_plugins = $2 where id = $1")
        .bind(id)
        .bind(names)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}

/// Set the job's error message without changing its state.
///
/// # Errors
///
/// Returns [`JobError::UpdateFailed`] when the update fails.
pub async fn set_job_error_message(
    conn: impl PgExecutor<'_>,
    id: i64,
    message: &str,
) -> Result<()> {
    sqlx::query("update jobs set error_message = $2 where id = $1")
        .bind(id)
        .bind(message)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("job {id}"), e))?;
    Ok(())
}
