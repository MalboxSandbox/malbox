use malbox_database::error::DatabaseError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    /// The database schema does not match this binary (incompatible older
    /// release, or a newer one); the message is printed verbatim so
    /// operators see the recovery step.
    #[error("{0}")]
    IncompatibleSchema(#[source] DatabaseError),

    #[error("Database initialization failed: {0}")]
    Database(#[source] DatabaseError),
}

impl From<DatabaseError> for DaemonError {
    fn from(e: DatabaseError) -> Self {
        match e {
            DatabaseError::IncompatibleSchema { .. } | DatabaseError::NewerSchema { .. } => {
                DaemonError::IncompatibleSchema(e)
            }
            other => DaemonError::Database(other),
        }
    }
}

pub type Result<T> = std::result::Result<T, DaemonError>;
