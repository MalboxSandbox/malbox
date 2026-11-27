use malbox_database::error::DatabaseError;
use malbox_plugin_internal::manager::error::ManagerError;
use malbox_resources::ResourceError;
use malbox_utils::storage::error::StorageError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("resource error: {0}")]
    Resource(#[from] ResourceError),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("plugin manager error: {0}")]
    Manager(#[from] ManagerError),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<malbox_database::Error> for SchedulerError {
    fn from(e: malbox_database::Error) -> Self {
        Self::Database(DatabaseError::from(e))
    }
}

pub type Result<T, E = SchedulerError> = std::result::Result<T, E>;
