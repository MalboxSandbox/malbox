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

/// Row from the `task_results` table.
#[derive(FromRow, Debug, Clone)]
pub struct TaskResult {
    pub id: i32,
    pub task_id: i32,
    pub plugin_name: String,
    pub result_name: String,
    pub format: ResultFormat,
    pub size_bytes: i64,
    pub file_path: String,
    pub created_on: OffsetDateTime,
}

/// Insert a new task result row.
pub async fn insert_task_result(
    pool: &PgPool,
    task_id: i32,
    plugin_name: &str,
    result_name: &str,
    format: ResultFormat,
    size_bytes: i64,
    file_path: &str,
) -> Result<TaskResult> {
    query_as!(
        TaskResult,
        r#"
        INSERT INTO task_results (task_id, plugin_name, result_name, format, size_bytes, file_path)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id, task_id, plugin_name, result_name,
                  format AS "format: ResultFormat",
                  size_bytes, file_path, created_on
        "#,
        task_id,
        plugin_name,
        result_name,
        format as ResultFormat,
        size_bytes,
        file_path,
    )
    .fetch_one(pool)
    .await
    .map_err(|e| {
        TaskResultError::InsertFailed {
            task_id,
            message: format!("{}:{}", plugin_name, result_name),
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
