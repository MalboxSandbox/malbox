use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("{0}")]
    Machine(#[from] MachineError),
    #[error("{0}")]
    Task(#[from] TaskError),
    #[error("{0}")]
    Sample(#[from] SampleError),
    #[error("{0}")]
    Image(#[from] ImageError),
    #[error("{0}")]
    Snapshot(#[from] SnapshotError),
    #[error("{0}")]
    ProvisionRun(#[from] ProvisionRunError),
    #[error("{0}")]
    TaskResult(#[from] TaskResultError),
    #[error("{0}")]
    Recipe(#[from] RecipeError),
    #[error("{0}")]
    CustomTransform(#[from] CustomTransformError),
}

#[derive(Error, Debug)]
pub enum MachineError {
    #[error("Failed to insert machine '{name}': {message}: {source}")]
    InsertFailed {
        name: String,
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to delete from `machines` table: {source}")]
    DeleteFailed {
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch machines: {source}")]
    FetchFailed {
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to update machine: {message}: {source}")]
    UpdateFailed {
        message: String,
        #[source]
        source: sqlx::Error,
    },
}

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Failed to insert task '{name}': {message}: {source}")]
    InsertFailed {
        name: String,
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch tasks: {message}: {source}")]
    FetchFailed {
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to update task {task_id}: {message}: {source}")]
    UpdateFailed {
        task_id: i32,
        message: String,
        #[source]
        source: sqlx::Error,
    },
}

#[derive(Error, Debug)]
pub enum SampleError {
    #[error("Failed to insert sample '{hash}': {message}: {source}")]
    InsertFailed {
        hash: String,
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch sample '{hash}': {message}: {source}")]
    FetchFailed {
        hash: String,
        message: String,
        #[source]
        source: sqlx::Error,
    },
}

#[derive(Error, Debug)]
pub enum ImageError {
    #[error("Failed to insert image '{name}': {message}: {source}")]
    InsertFailed {
        name: String,
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch image: {source}")]
    FetchFailed {
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to update image: {message}: {source}")]
    UpdateFailed {
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to delete image '{name}': {source}")]
    DeleteFailed {
        name: String,
        #[source]
        source: sqlx::Error,
    },
}

#[derive(Error, Debug)]
pub enum SnapshotError {
    #[error("Failed to insert snapshot '{name}' for machine {machine_id}: {source}")]
    InsertFailed {
        name: String,
        machine_id: i32,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch snapshots: {source}")]
    FetchFailed {
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to update snapshot: {message}: {source}")]
    UpdateFailed {
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to delete snapshot: {source}")]
    DeleteFailed {
        #[source]
        source: sqlx::Error,
    },
}

#[derive(Error, Debug)]
pub enum ProvisionRunError {
    #[error("Failed to insert provision run for machine {machine_id}: {source}")]
    InsertFailed {
        machine_id: i32,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to update provision run: {message}: {source}")]
    UpdateFailed {
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch provision runs: {source}")]
    FetchFailed {
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to delete provision runs: {source}")]
    DeleteFailed {
        #[source]
        source: sqlx::Error,
    },
}

#[derive(Error, Debug)]
pub enum TaskResultError {
    #[error("Failed to insert result for task {task_id} ({message}): {source}")]
    InsertFailed {
        task_id: i32,
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch results for task {task_id}: {source}")]
    FetchFailed {
        task_id: i32,
        #[source]
        source: sqlx::Error,
    },
}

#[derive(Error, Debug)]
pub enum RecipeError {
    #[error("Failed to insert recipe '{name}': {message}: {source}")]
    InsertFailed {
        name: String,
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch recipe: {source}")]
    FetchFailed {
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to update recipe: {message}: {source}")]
    UpdateFailed {
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to delete recipe: {source}")]
    DeleteFailed {
        #[source]
        source: sqlx::Error,
    },
}

#[derive(Error, Debug)]
pub enum CustomTransformError {
    #[error("Failed to insert transform '{name}': {message}: {source}")]
    InsertFailed {
        name: String,
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to fetch transform: {source}")]
    FetchFailed {
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to update transform: {message}: {source}")]
    UpdateFailed {
        message: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("Failed to delete transform: {source}")]
    DeleteFailed {
        #[source]
        source: sqlx::Error,
    },
}

pub type Result<T> = std::result::Result<T, DatabaseError>;
