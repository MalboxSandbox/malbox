use crate::error::{Result, TaskError};
use malbox_database::PgPool;
use malbox_database::repositories::tasks::{
    Task, TaskState, fetch_pending_tasks, fetch_task, insert_task, reset_orphaned_tasks,
    update_task_status,
};
use std::collections::HashMap;
use time::OffsetDateTime;
use time::PrimitiveDateTime;
use tokio::sync::RwLock;

/// The TaskStore is responsible for storing tasks and synchronizing
/// with the database.
pub struct TaskStore {
    // Persistent database connection pool.
    db: PgPool,
    // In-memory cache of tasks for quick access.
    // Using RwLock for concurrent read/write access.
    tasks: RwLock<HashMap<i32, Task>>,
}

impl TaskStore {
    /// Creates a new TaskStore.
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            tasks: RwLock::new(HashMap::new()),
        }
    }

    /// Get a reference to the database pool.
    pub fn pool(&self) -> &PgPool {
        &self.db
    }

    /// Load a task by ID, first checking the in-memory cache,
    /// then falling back to the database if needed.
    pub async fn load_task(&self, task_id: i32) -> Result<Task> {
        // First check the in-memory cache with a read lock.
        // This allows multiple readers but blocks writers.
        {
            let tasks = self.tasks.read().await;
            if let Some(task) = tasks.get(&task_id) {
                // Returning a clone to avoid holding the lock longer than needed.
                return Ok(task.clone());
            }
        }

        // Not found in cache, fetch from the database.
        let task = fetch_task(&self.db, task_id)
            .await?
            .ok_or_else(|| TaskError::NotFound(task_id.to_string()))?;

        // Update the cache with a write lock.
        {
            let mut tasks = self.tasks.write().await;
            tasks.insert(task.id.unwrap(), task.clone());
        }

        Ok(task)
    }

    /// Update the state of a task both in memory and database.
    pub async fn update_task_state(&self, task_id: i32, state: TaskState) -> Result<()> {
        // Update the in-memory cache.
        {
            let mut tasks = self.tasks.write().await;
            if let Some(task) = tasks.get_mut(&task_id) {
                // Update the task's state.
                task.status = state.clone();

                // Update timestamps based on the new state.
                match &state {
                    TaskState::Running => {
                        let now_odt = OffsetDateTime::now_utc();
                        task.started_on =
                            Some(PrimitiveDateTime::new(now_odt.date(), now_odt.time()));
                    }
                    // NOTE: Should we actually consider a failed task as completed in our cache?
                    TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                        let now_odt = OffsetDateTime::now_utc();
                        task.completed_on =
                            Some(PrimitiveDateTime::new(now_odt.date(), now_odt.time()));
                    }
                    _ => {}
                }
            }
        }

        // Update task state in the database.
        update_task_status(&self.db, task_id, state).await.unwrap();

        Ok(())
    }

    /// Update the result of a task both in-memory and database.
    ///
    /// Note: The Task struct does not yet have a result field.
    /// This will be implemented when the result storage schema is added.
    pub async fn update_task_result(&self, _task_id: i32, _result: String) -> Result<()> {
        // TODO: Add result field to Task struct and persist to DB
        Ok(())
    }

    /// Cache a task in memory without inserting into the database.
    ///
    /// Used by the scheduler ingestion loop when tasks are already
    /// persisted by the HTTP layer.
    pub async fn cache_task(&self, task: Task) {
        if let Some(id) = task.id {
            let mut tasks = self.tasks.write().await;
            tasks.insert(id, task);
        }
    }

    /// Reset tasks stranded in transient states from a prior daemon run.
    ///
    /// Tasks marked Running/Initializing/PreparingResources/Stopping in the DB
    /// but not known to any live worker are leftovers from a crash — mark them
    /// Failed so the state is truthful. Returns the number of rows reset.
    pub async fn recover_orphaned_tasks(&self) -> Result<u64> {
        Ok(reset_orphaned_tasks(&self.db).await?)
    }

    /// Load all pending tasks from the database.
    /// This is used during startup to initialize the task queue.
    pub async fn load_pending_tasks(&self) -> Result<Vec<Task>> {
        // Fetch the pending tasks from database.
        let pending_tasks = fetch_pending_tasks(&self.db).await?;
        // Update in-memory cache with pending tasks fetched from database.
        {
            let mut tasks_map = self.tasks.write().await;
            for task in &pending_tasks {
                tasks_map.insert(task.id.unwrap(), task.clone());
            }
        }

        Ok(pending_tasks)
    }

    /// Store a new task, both in-memory and database.
    pub async fn store_task(&self, task: Task) -> Result<()> {
        // First insert the task in the database.
        // We need the ID that postgres generates.
        let task = insert_task(&self.db, task).await?;

        // Add the task to in-memory storage.
        {
            let mut tasks_map = self.tasks.write().await;
            tasks_map.insert(task.id.unwrap(), task.clone());
        }

        Ok(())
    }
}
