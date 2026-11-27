//! `task_artifacts` and `artifact_edges`: the per-task artifact DAG.
//!
//! Enum columns (`state`, `relation`) travel as text: they are cast to text
//! when read and from text when written, so callers use the shared literal
//! strings (`"stored"`, `"extracted"`, ...).

use crate::error::{ArtifactError, DatabaseError, Result};
use serde::Serialize;
use sqlx::{AssertSqlSafe, FromRow, PgConnection, PgExecutor, PgPool};
use std::collections::HashMap;
use std::fmt;
use time::OffsetDateTime;

/// Every column of `task_artifacts`, enum columns cast to text.
pub(crate) const ARTIFACT_COLUMNS: &str = "id, task_id, sha256, size_bytes, storage_path, \"analyze\", \
    sample_id, depth, artifact_type, mime, state::text as state, drop_reason, priority, deadline, \
    canceled_by, promoted_task_id, dynamic_recursion_bypass, analyze_requested, created_on";

/// Every column of `artifact_edges`, enum columns cast to text.
const EDGE_COLUMNS: &str = "task_id, parent_artifact_id, child_artifact_id, \
    relation::text as relation, source_job_id, source_plugin, name, created_on";

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct TaskArtifact {
    pub id: i64,
    pub task_id: i32,
    pub sha256: String,
    pub size_bytes: i64,
    /// Blob path relative to the data directory; `None` when the
    /// `max_total_bytes` limit refused the blob.
    pub storage_path: Option<String>,
    pub analyze: bool,
    pub sample_id: Option<i64>,
    pub depth: i32,
    pub artifact_type: String,
    pub mime: String,
    pub state: String,
    pub drop_reason: Option<String>,
    pub priority: i64,
    pub deadline: Option<OffsetDateTime>,
    pub canceled_by: Option<String>,
    pub promoted_task_id: Option<i32>,
    pub dynamic_recursion_bypass: bool,
    pub analyze_requested: bool,
    pub created_on: OffsetDateTime,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ArtifactEdge {
    pub task_id: i32,
    pub parent_artifact_id: i64,
    pub child_artifact_id: i64,
    pub relation: String,
    pub source_job_id: Option<i64>,
    pub source_plugin: Option<String>,
    pub name: Option<String>,
    pub created_on: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct NewArtifact<'a> {
    pub task_id: i32,
    pub sha256: &'a str,
    pub size_bytes: i64,
    pub storage_path: Option<&'a str>,
    pub analyze: bool,
    pub sample_id: Option<i64>,
    pub depth: i32,
    pub artifact_type: &'a str,
    pub mime: &'a str,
    pub state: &'a str,
    pub drop_reason: Option<&'a str>,
    pub priority: i64,
    pub analyze_requested: bool,
    pub dynamic_recursion_bypass: bool,
}

fn insert_err(target: fmt::Arguments<'_>, source: sqlx::Error) -> DatabaseError {
    ArtifactError::InsertFailed {
        target: target.to_string(),
        source,
    }
    .into()
}

fn fetch_err(target: fmt::Arguments<'_>, source: sqlx::Error) -> DatabaseError {
    ArtifactError::FetchFailed {
        target: target.to_string(),
        source,
    }
    .into()
}

fn update_err(target: fmt::Arguments<'_>, source: sqlx::Error) -> DatabaseError {
    ArtifactError::UpdateFailed {
        target: target.to_string(),
        source,
    }
    .into()
}

/// Insert an artifact, or return the existing row with the same
/// `(task_id, sha256)`. The boolean is `true` when the row was created.
pub async fn insert_or_get_artifact(
    conn: &mut PgConnection,
    a: &NewArtifact<'_>,
) -> Result<(TaskArtifact, bool)> {
    let sql = format!(
        "insert into task_artifacts (task_id, sha256, size_bytes, storage_path, \"analyze\", \
         sample_id, depth, artifact_type, mime, state, drop_reason, priority, \
         analyze_requested, dynamic_recursion_bypass) \
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10::artifact_state, $11, $12, $13, $14) \
         on conflict (task_id, sha256) do nothing returning {ARTIFACT_COLUMNS}"
    );
    let inserted = sqlx::query_as::<_, TaskArtifact>(AssertSqlSafe(sql))
        .bind(a.task_id)
        .bind(a.sha256)
        .bind(a.size_bytes)
        .bind(a.storage_path)
        .bind(a.analyze)
        .bind(a.sample_id)
        .bind(a.depth)
        .bind(a.artifact_type)
        .bind(a.mime)
        .bind(a.state)
        .bind(a.drop_reason)
        .bind(a.priority)
        .bind(a.analyze_requested)
        .bind(a.dynamic_recursion_bypass)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| {
            insert_err(
                format_args!("artifact {} of task {}", a.sha256, a.task_id),
                e,
            )
        })?;
    if let Some(row) = inserted {
        return Ok((row, true));
    }

    let sql =
        format!("select {ARTIFACT_COLUMNS} from task_artifacts where task_id = $1 and sha256 = $2");
    let existing = sqlx::query_as::<_, TaskArtifact>(AssertSqlSafe(sql))
        .bind(a.task_id)
        .bind(a.sha256)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| {
            fetch_err(
                format_args!("artifact {} of task {}", a.sha256, a.task_id),
                e,
            )
        })?;
    Ok((existing, false))
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_edge(
    conn: impl PgExecutor<'_>,
    task_id: i32,
    parent: i64,
    child: i64,
    relation: &str,
    source_job_id: Option<i64>,
    source_plugin: Option<&str>,
    name: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "insert into artifact_edges (task_id, parent_artifact_id, child_artifact_id, relation, \
         source_job_id, source_plugin, name) \
         values ($1, $2, $3, $4::artifact_relation, $5, $6, $7) on conflict do nothing",
    )
    .bind(task_id)
    .bind(parent)
    .bind(child)
    .bind(relation)
    .bind(source_job_id)
    .bind(source_plugin)
    .bind(name)
    .execute(conn)
    .await
    .map_err(|e| insert_err(format_args!("edge {parent} -> {child}"), e))?;
    Ok(())
}

/// Record that a plugin asked for this artifact to be analysed. `bypass`
/// only ever turns `dynamic_recursion_bypass` on.
pub async fn set_analyze_requested(conn: impl PgExecutor<'_>, id: i64, bypass: bool) -> Result<()> {
    sqlx::query(
        "update task_artifacts set analyze_requested = true, \
         dynamic_recursion_bypass = dynamic_recursion_bypass or $2 where id = $1",
    )
    .bind(id)
    .bind(bypass)
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("artifact {id}"), e))?;
    Ok(())
}

/// Root artifact id per task, for the given task ids.
pub async fn fetch_root_artifact_ids(pool: &PgPool, task_ids: &[i32]) -> Result<HashMap<i32, i64>> {
    let rows: Vec<(i32, i64)> = sqlx::query_as(
        "select task_id, id from task_artifacts where task_id = any($1) and depth = 0",
    )
    .bind(task_ids)
    .fetch_all(pool)
    .await
    .map_err(|e| fetch_err(format_args!("root artifacts of tasks {task_ids:?}"), e))?;
    Ok(rows.into_iter().collect())
}

pub async fn fetch_artifact(pool: &PgPool, id: i64) -> Result<Option<TaskArtifact>> {
    let sql = format!("select {ARTIFACT_COLUMNS} from task_artifacts where id = $1");
    sqlx::query_as::<_, TaskArtifact>(AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| fetch_err(format_args!("artifact {id}"), e))
}

pub async fn fetch_task_artifacts(pool: &PgPool, task_id: i32) -> Result<Vec<TaskArtifact>> {
    let sql =
        format!("select {ARTIFACT_COLUMNS} from task_artifacts where task_id = $1 order by id");
    sqlx::query_as::<_, TaskArtifact>(AssertSqlSafe(sql))
        .bind(task_id)
        .fetch_all(pool)
        .await
        .map_err(|e| fetch_err(format_args!("artifacts of task {task_id}"), e))
}

/// Every edge of a task, oldest first.
pub async fn fetch_task_edges(pool: &PgPool, task_id: i32) -> Result<Vec<ArtifactEdge>> {
    let sql = format!(
        "select {EDGE_COLUMNS} from artifact_edges where task_id = $1 \
         order by created_on, parent_artifact_id, child_artifact_id"
    );
    sqlx::query_as::<_, ArtifactEdge>(AssertSqlSafe(sql))
        .bind(task_id)
        .fetch_all(pool)
        .await
        .map_err(|e| fetch_err(format_args!("edges of task {task_id}"), e))
}

/// Edges emitted by a job, each paired with its child artifact.
pub async fn fetch_edges_from_job(
    pool: &PgPool,
    job_id: i64,
) -> Result<Vec<(ArtifactEdge, TaskArtifact)>> {
    let sql = format!(
        "select {EDGE_COLUMNS} from artifact_edges where source_job_id = $1 \
         order by created_on, child_artifact_id"
    );
    let edges = sqlx::query_as::<_, ArtifactEdge>(AssertSqlSafe(sql))
        .bind(job_id)
        .fetch_all(pool)
        .await
        .map_err(|e| fetch_err(format_args!("edges from job {job_id}"), e))?;
    if edges.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<i64> = edges.iter().map(|e| e.child_artifact_id).collect();
    let sql = format!("select {ARTIFACT_COLUMNS} from task_artifacts where id = any($1)");
    let children: HashMap<i64, TaskArtifact> =
        sqlx::query_as::<_, TaskArtifact>(AssertSqlSafe(sql))
            .bind(&ids)
            .fetch_all(pool)
            .await
            .map_err(|e| fetch_err(format_args!("children of job {job_id}"), e))?
            .into_iter()
            .map(|a| (a.id, a))
            .collect();

    Ok(edges
        .into_iter()
        .filter_map(|e| {
            let child = children.get(&e.child_artifact_id).cloned()?;
            Some((e, child))
        })
        .collect())
}

pub async fn fetch_root_artifact(pool: &PgPool, task_id: i32) -> Result<Option<TaskArtifact>> {
    let sql = format!(
        "select {ARTIFACT_COLUMNS} from task_artifacts where task_id = $1 and depth = 0 \
         order by id limit 1"
    );
    sqlx::query_as::<_, TaskArtifact>(AssertSqlSafe(sql))
        .bind(task_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| fetch_err(format_args!("root artifact of task {task_id}"), e))
}

/// Set an artifact's state together with its drop reason and canceled-by
/// marker (both written as given).
pub async fn update_artifact_state(
    conn: impl PgExecutor<'_>,
    id: i64,
    state: &str,
    drop_reason: Option<&str>,
    canceled_by: Option<&str>,
) -> Result<()> {
    sqlx::query(
        "update task_artifacts set state = $2::artifact_state, drop_reason = $3, \
         canceled_by = $4 where id = $1",
    )
    .bind(id)
    .bind(state)
    .bind(drop_reason)
    .bind(canceled_by)
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("artifact {id}"), e))?;
    Ok(())
}

/// Turn a stored artifact into an analysed one.
pub async fn upgrade_artifact_to_analysed(
    conn: impl PgExecutor<'_>,
    id: i64,
    sample_id: i64,
    depth: i32,
) -> Result<()> {
    sqlx::query(
        "update task_artifacts set \"analyze\" = true, sample_id = $2, state = 'pending', \
         drop_reason = null, depth = least(depth, $3) where id = $1",
    )
    .bind(id)
    .bind(sample_id)
    .bind(depth)
    .execute(conn)
    .await
    .map_err(|e| update_err(format_args!("artifact {id}"), e))?;
    Ok(())
}

/// A limit refused analysis; `analyze` stays false.
pub async fn set_artifact_admission_dropped(
    conn: impl PgExecutor<'_>,
    id: i64,
    reason: &str,
) -> Result<()> {
    sqlx::query("update task_artifacts set state = 'dropped', drop_reason = $2 where id = $1")
        .bind(id)
        .bind(reason)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("artifact {id}"), e))?;
    Ok(())
}

/// Lock the task row and return its shared data. Use inside a transaction
/// together with [`set_task_shared_data`].
pub async fn fetch_task_shared_data_for_update(
    conn: impl PgExecutor<'_>,
    task_id: i32,
) -> Result<serde_json::Value> {
    sqlx::query_scalar::<_, serde_json::Value>(
        "select shared_data from tasks where id = $1 for update",
    )
    .bind(task_id)
    .fetch_one(conn)
    .await
    .map_err(|e| fetch_err(format_args!("shared data of task {task_id}"), e))
}

pub async fn set_task_shared_data(
    conn: impl PgExecutor<'_>,
    task_id: i32,
    data: &serde_json::Value,
) -> Result<()> {
    sqlx::query("update tasks set shared_data = $2 where id = $1")
        .bind(task_id)
        .bind(data)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("shared data of task {task_id}"), e))?;
    Ok(())
}

pub async fn set_artifact_priority(
    conn: impl PgExecutor<'_>,
    id: i64,
    priority: i64,
) -> Result<()> {
    sqlx::query("update task_artifacts set priority = $2 where id = $1")
        .bind(id)
        .bind(priority)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("artifact {id}"), e))?;
    Ok(())
}

pub async fn set_artifact_deadline(
    conn: impl PgExecutor<'_>,
    id: i64,
    deadline: Option<OffsetDateTime>,
) -> Result<()> {
    sqlx::query("update task_artifacts set deadline = $2 where id = $1")
        .bind(id)
        .bind(deadline)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("artifact {id}"), e))?;
    Ok(())
}

pub async fn set_promoted_task(conn: impl PgExecutor<'_>, id: i64, task_id: i32) -> Result<()> {
    sqlx::query("update task_artifacts set promoted_task_id = $2 where id = $1")
        .bind(id)
        .bind(task_id)
        .execute(conn)
        .await
        .map_err(|e| update_err(format_args!("artifact {id}"), e))?;
    Ok(())
}

/// Lock the task row and return the bytes stored for it (artifacts whose
/// blob was kept). Call inside the transaction that inserts the task's
/// artifacts, so concurrent ingests cannot each pass the
/// `max_total_bytes` check against the same total.
///
/// # Errors
///
/// Returns an artifact fetch error when either query fails.
pub async fn lock_task_artifact_bytes(conn: &mut PgConnection, task_id: i32) -> Result<i64> {
    sqlx::query("select 1 from tasks where id = $1 for update")
        .bind(task_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| fetch_err(format_args!("task {task_id}"), e))?;
    sqlx::query_scalar::<_, i64>(
        "select coalesce(sum(size_bytes), 0)::bigint from task_artifacts \
         where task_id = $1 and storage_path is not null",
    )
    .bind(task_id)
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| fetch_err(format_args!("stored bytes of task {task_id}"), e))
}

/// Where a sample appears as a non-root artifact: `(task_id, artifact_id, depth)`.
pub async fn fetch_sample_appearances(
    pool: &PgPool,
    sample_id: i64,
) -> Result<Vec<(i32, i64, i32)>> {
    sqlx::query_as::<_, (i32, i64, i32)>(
        "select task_id, id, depth from task_artifacts where sample_id = $1 and depth > 0 \
         order by task_id desc, id",
    )
    .bind(sample_id)
    .fetch_all(pool)
    .await
    .map_err(|e| fetch_err(format_args!("appearances of sample {sample_id}"), e))
}
