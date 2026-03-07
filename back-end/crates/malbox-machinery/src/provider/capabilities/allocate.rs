//! Allocate capability module.

use crate::machine::{Machine, MachineEndpoint, MachineSpec};
use async_trait::async_trait;
use std::error::Error;

/// Core capability: Allocate and deallocate machines.
///
/// Every provider must implement this capability in order to (de)allocate machines.
#[async_trait]
pub trait Allocate: Send + Sync {
    /// Allocate a new machine with the given specification.
    ///
    /// This should create a new machine (VM, container, etc.) according to the spec,
    /// start it, and return a Machine handle once it's ready.
    async fn allocate(&self, spec: &MachineSpec) -> Result<Machine, Box<dyn Error + Send + Sync>>;

    /// Deallocate an existing machine.
    ///
    /// This should stop the machine and clean up all resources.
    /// After this call, the machine should no longer exist.
    async fn deallocate(&self, machine: &Machine) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// List all machines currently managed by this provider.
    async fn list(&self) -> Result<Vec<Machine>, Box<dyn Error + Send + Sync>>;

    /// Resolve the network endpoint for an already-allocated machine.
    ///
    /// Returns `Ok(Some(endpoint))` if the endpoint is available, `Ok(None)` if the
    /// machine is not yet ready (e.g., still booting), or `Err` on failure.
    ///
    /// The default implementation returns `Ok(None)`. Providers should override this
    /// to query the machine's network address (e.g., via guest agent, DHCP lease).
    async fn resolve_endpoint(
        &self,
        _machine: &Machine,
    ) -> Result<Option<MachineEndpoint>, Box<dyn Error + Send + Sync>> {
        Ok(None)
    }
}
