//! This crate contains interfaces related to machinery (providers, provisioners, etc.)
//!
//! Contains machinery functionalities and provider interfaces. Actual implementations
//! are found in the different `malbox-provider-*` and `malbox-provisioner-*` crates.

pub mod machine;
pub mod provider;
pub mod provisioner;

pub use machine::{
    DiskType, Machine, MachineEndpoint, MachineId, MachineSpec, MachineState, Network, NetworkMode,
    Platform, Provisioning, Resources, Storage,
};
pub use provider::capabilities::{
    Allocate, Clone, ExecOptions, ExecResult, GuestAccess, GuestSession, GuestStatus, Migrate,
    Snapshot, SnapshotId, SnapshotInfo,
};
pub use provider::{
    ProviderHandle, ProviderMetadata, create_provider, get_provider_metadata, list_providers,
};
pub use provisioner::{
    ProvisionContext, ProvisionResult, ProvisionStatus, Provisioner, ProvisionerMetadata,
    create_provisioner, get_provisioner_metadata, list_provisioners,
};
