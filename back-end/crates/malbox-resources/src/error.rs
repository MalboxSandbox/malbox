use thiserror::Error;

#[derive(Debug, Error)]
pub enum ResourceError {
    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Provisioner error: {0}")]
    Provisioner(String),

    #[error("Pool exhausted: no available machines")]
    PoolExhausted,

    #[error("Machine not found: {id}")]
    MachineNotFound { id: String },

    #[error("Snapshot not found: {id}")]
    SnapshotNotFound { id: String },

    #[error("Machine not ready: {reason}")]
    MachineNotReady { reason: String },

    #[error("Timeout waiting for machine: {0}")]
    Timeout(String),

    #[error("Allocation failed: {0}")]
    AllocationFailed(String),

    #[error("Deallocation failed: {0}")]
    DeallocationFailed(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error(
        "Unknown transport '{name}': not supported by provider '{provider}'. Available: {available:?}"
    )]
    UnknownTransport {
        name: String,
        provider: String,
        available: Vec<String>,
    },

    #[error("Incompatible transport '{transport}': {reason}")]
    IncompatibleTransport { transport: String, reason: String },

    #[error("Guest access error: {0}")]
    GuestAccess(String),

    #[error("Guest access not configured but required by task")]
    GuestAccessNotConfigured,

    #[error("Database error: {0}")]
    Database(String),
}

pub type Result<T> = std::result::Result<T, ResourceError>;
