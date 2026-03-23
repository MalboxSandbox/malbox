//! Machinery interfaces — providers, provisioners, and machine types.
//!
//! This crate defines the traits and types that provider and provisioner
//! implementations depend on. Actual implementations live in separate
//! `malbox-provider-*` and `malbox-provisioner-*` crates.

pub mod machine;
pub mod provider;
pub mod provisioner;

pub use machine::{
    Arch, CreateMachineParams, Machine, MachineEndpoint, MachineId, MachineState, Platform,
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
