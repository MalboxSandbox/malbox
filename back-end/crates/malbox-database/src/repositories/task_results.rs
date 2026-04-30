use crate::error::{Result, TaskResultError};
use sqlx::{FromRow, PgPool, query_as};
use time::OffsetDateTime;

/// Mirrors the `result_format` Postgres enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "result_format", rename_all = "lowercase")]
pub enum ResultFormat {
    Json,
    Bytes,
}

/// Mirrors the `task_result_role` Postgres enum. A task output is either a
/// structured `Report` envelope (JSON with `result_name = "report"`) or an
/// opaque artifact (any other output).
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "task_result_role", rename_all = "lowercase")]
pub enum ResultRole {
    Report,
    Artifact,
}

/// Row from the `task_results` table.
#[derive(FromRow, Debug, Clone)]
pub struct TaskResult {
    pub id: i32,
    pub task_id: i32,
    pub plugin_name: String,
    pub result_name: String,
    pub format: ResultFormat,
    pub role: ResultRole,
    pub size_bytes: i64,
    pub file_path: String,
    pub created_on: OffsetDateTime,
}

pub struct InsertTaskResult<'a> {
    pub task_id: i32,
    pub plugin_name: &'a str,
    pub result_name: &'a str,
    pub format: ResultFormat,
    pub role: ResultRole,
    pub size_bytes: i64,
    pub file_path: &'a str,
}

/// Insert a new task result row.
pub async fn insert_task_result(
    pool: &PgPool,
    params: &InsertTaskResult<'_>,
) -> Result<TaskResult> {
    query_as!(
        TaskResult,
        r#"
        INSERT INTO task_results (task_id, plugin_name, result_name, format, role, size_bytes, file_path)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, task_id, plugin_name, result_name,
                  format AS "format: ResultFormat",
                  role   AS "role: ResultRole",
                  size_bytes, file_path, created_on
        "#,
        params.task_id,
        params.plugin_name,
        params.result_name,
        params.format as ResultFormat,
        params.role as ResultRole,
        params.size_bytes,
        params.file_path,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        TaskResultError::InsertFailed {
            task_id: params.task_id,
            message: format!("{}:{}", params.plugin_name, params.result_name),
            source: e,
        }
        .into()
    })
}

/// Fetch a single result row by its id.
pub async fn fetch_task_result(pool: &PgPool, result_id: i32) -> Result<Option<TaskResult>> {
    query_as!(
        TaskResult,
        r#"
        SELECT id, task_id, plugin_name, result_name,
               format AS "format: ResultFormat",
               role   AS "role: ResultRole",
               size_bytes, file_path, created_on
        FROM task_results
        WHERE id = $1
        "#,
        result_id,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        TaskResultError::FetchFailed {
            task_id: result_id,
            source: e,
        }
        .into()
    })
}

/// Fetch all results for a given task.
pub async fn fetch_task_results(pool: &PgPool, task_id: i32) -> Result<Vec<TaskResult>> {
    query_as!(
        TaskResult,
        r#"
        SELECT id, task_id, plugin_name, result_name,
               format AS "format: ResultFormat",
               role   AS "role: ResultRole",
               size_bytes, file_path, created_on
        FROM task_results
        WHERE task_id = $1
        ORDER BY created_on
        "#,
        task_id,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| TaskResultError::FetchFailed { task_id, source: e }.into())
}

/// Fetch only the `report` rows for a task. Uses the `(task_id, role)` index.
pub async fn fetch_task_reports(pool: &PgPool, task_id: i32) -> Result<Vec<TaskResult>> {
    query_as!(
        TaskResult,
        r#"
        SELECT id, task_id, plugin_name, result_name,
               format AS "format: ResultFormat",
               role   AS "role: ResultRole",
               size_bytes, file_path, created_on
        FROM task_results
        WHERE task_id = $1 AND role = 'report'::task_result_role
        ORDER BY created_on
        "#,
        task_id,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| TaskResultError::FetchFailed { task_id, source: e }.into())
}
