use thiserror::Error;

/// Failure raised by machine pool, provisioning and transport operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ResourceError {
    /// The machinery provider reported an error.
    #[error("Provider error: {0}")]
    Provider(String),
    /// A provisioner failed to create or run.
    #[error("Provisioner error: {0}")]
    Provisioner(String),
    /// Every machine in the pool is busy.
    #[error("Pool exhausted: no available machines")]
    PoolExhausted,
    /// No machine with the given identifier exists.
    #[error("Machine not found: {id}")]
    MachineNotFound {
        /// Identifier that was looked up.
        id: String,
    },
    /// No snapshot with the given identifier exists.
    #[error("Snapshot not found: {id}")]
    SnapshotNotFound {
        /// Identifier that was looked up.
        id: String,
    },
    /// The machine exists but cannot be used yet.
    #[error("Machine not ready: {reason}")]
    MachineNotReady {
        /// Why the machine is not ready.
        reason: String,
    },
    /// A wait on the machine exceeded its deadline.
    #[error("Timeout waiting for machine: {0}")]
    Timeout(String),
    /// Assigning a machine failed.
    #[error("Allocation failed: {0}")]
    AllocationFailed(String),
    /// Releasing a machine failed.
    #[error("Deallocation failed: {0}")]
    DeallocationFailed(String),
    /// Unexpected internal failure.
    #[error("Internal error: {0}")]
    Internal(String),
    /// The configured guest transport is not offered by the provider or gRPC.
    #[error(
        "Unknown transport '{name}': not supported by provider '{provider}'. Available: {available:?}"
    )]
    UnknownTransport {
        /// Configured transport name.
        name: String,
        /// Name of the active provider.
        provider: String,
        /// Transports that would have been accepted.
        available: Vec<String>,
    },
    /// The transport exists but conflicts with the deployment (e.g. network mode).
    #[error("Incompatible transport '{transport}': {reason}")]
    IncompatibleTransport {
        /// Configured transport name.
        transport: String,
        /// Why it cannot be used.
        reason: String,
    },
    /// Communicating with the guest failed.
    #[error("Guest access error: {0}")]
    GuestAccess(String),
    /// The task needs guest access but none is configured.
    #[error("Guest access not configured but required by task")]
    GuestAccessNotConfigured,
    /// A database query failed.
    #[error("Database error: {0}")]
    Database(#[source] malbox_database::error::DatabaseError),
    /// No machine matches the requested platform.
    #[error("No machine available for platform {platform}")]
    NoMachineAvailable {
        /// Requested platform name.
        platform: String,
    },
    /// The machine is leased to a task and cannot be modified.
    #[error("Machine {id} is currently assigned to a task")]
    MachineAssigned {
        /// Database ID of the machine.
        id: i32,
    },
    /// The machine is not in the state the operation requires.
    #[error("Machine {id} is in state '{status}', expected '{expected}'")]
    InvalidMachineState {
        /// Database ID of the machine.
        id: i32,
        /// Current machine status.
        status: String,
        /// Status the operation requires.
        expected: String,
    },
    /// Reverting the machine to its snapshot failed.
    #[error("Snapshot restore failed for machine {id}: {reason}")]
    SnapshotRestoreFailed {
        /// Database ID of the machine.
        id: i32,
        /// Failure description.
        reason: String,
    },
}

/// Result alias using [`ResourceError`].
pub type Result<T, E = ResourceError> = std::result::Result<T, E>;
