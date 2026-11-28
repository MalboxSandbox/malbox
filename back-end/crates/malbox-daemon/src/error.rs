use malbox_database::error::DatabaseError;
use thiserror::Error;

/// Failures that abort daemon startup or shutdown.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum DaemonError {
    /// A subsystem failed to start or stop; the message describes which and why.
    #[error("Internal error: {0}")]
    Internal(String),

    /// The configuration file is invalid or inconsistent.
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// The database schema does not match this binary (incompatible older
    /// release, or a newer one); the message is printed verbatim so
    /// operators see the recovery step.
    #[error("{0}")]
    IncompatibleSchema(#[source] DatabaseError),

    /// Connecting to or migrating the database failed.
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

/// Result alias for daemon operations failing with [`DaemonError`].
pub type Result<T> = std::result::Result<T, DaemonError>;
