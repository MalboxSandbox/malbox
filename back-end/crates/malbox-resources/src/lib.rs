//! Resource management for Malbox
//!
//! This crate provides high-level resource management for machines (VMs, bare-metal, containers, etc.),
//! orchestrating providers and provisioners from `malbox-machinery`.

pub mod error;
pub mod pool;
pub mod transport;

pub use error::{ResourceError, Result};
pub use pool::{CreateMachineRequest, MachinePool, MachinePoolConfig};
pub use transport::{ResolvedTransport, resolve_transport};
