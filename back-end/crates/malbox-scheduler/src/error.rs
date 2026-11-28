use malbox_database::error::DatabaseError;
use malbox_plugin_internal::manager::error::ManagerError;
use malbox_resources::ResourceError;
use malbox_utils::storage::error::StorageError;
use thiserror::Error;

/// Failures inside the orchestrator: storage, database, resources or plugin management.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum SchedulerError {
    /// A database query or migration failed.
    #[error("database error: {0}")]
    Database(#[from] DatabaseError),
    /// Acquiring or releasing an analysis machine failed.
    #[error("resource error: {0}")]
    Resource(#[from] ResourceError),
    /// Reading or writing the sample or result store failed.
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    /// The plugin manager could not run or reach a plugin.
    #[error("plugin manager error: {0}")]
    Manager(#[from] ManagerError),
    /// An orchestrator invariant was violated; the message describes it.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<malbox_database::Error> for SchedulerError {
    fn from(e: malbox_database::Error) -> Self {
        Self::Database(DatabaseError::from(e))
    }
}

/// Result alias for orchestrator operations, defaulting to [`SchedulerError`].
pub type Result<T, E = SchedulerError> = std::result::Result<T, E>;
