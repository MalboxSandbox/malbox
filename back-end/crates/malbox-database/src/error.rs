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

pub type Result<T> = std::result::Result<T, DatabaseError>;
