use thiserror::Error;

#[derive(Error, Debug)]
pub enum DaemonError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Database initialization failed: {0}")]
    Database(#[from] malbox_database::Error),
}

pub type Result<T> = std::result::Result<T, DaemonError>;
