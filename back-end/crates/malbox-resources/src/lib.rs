//! Resource management for Malbox
//!
//! This crate provides high-level resource management for machines (VMs, bare-metal, containers, etc.),
//! orchestrating providers and provisioners from `malbox-machinery`.

pub mod error;
pub mod manager;
pub mod pool;
pub mod transport;

pub use error::{ResourceError, Result};
pub use manager::{MachineryManager, OnDemandManager, PooledManager};
pub use pool::{MachinePool, PoolConfig, PoolStats};
pub use transport::{ResolvedTransport, resolve_transport};

// Re-export commonly used types from malbox-machinery
pub use malbox_machinery::{
    // Capabilities
    Allocate,
    Clone as CloneCapability,
    // Machine types
    DiskType,
    Machine,
    MachineEndpoint,
    MachineId,
    MachineSpec,
    MachineState,
    Migrate,
    Network,
    NetworkMode,
    Platform,
    // Provider registry
    ProviderHandle,
    ProviderMetadata,
    // Provisioner
    ProvisionContext,
    Provisioner,
    Resources,
    Snapshot,
    SnapshotId,
    SnapshotInfo,
    Storage,
};
