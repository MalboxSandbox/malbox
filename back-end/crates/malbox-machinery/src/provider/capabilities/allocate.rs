//! Allocate capability — core machine lifecycle management.

use crate::machine::{CreateMachineParams, Machine, MachineEndpoint};
use async_trait::async_trait;
use std::error::Error;

/// Every provider must implement this capability. It covers creating,
/// destroying, starting, stopping, listing, and resolving endpoints.
#[async_trait]
pub trait Allocate: Send + Sync {
    /// Create a new machine from the given parameters.
    ///
    /// Provisions resources but does **not** start
    /// the machine. Call [`start`] afterwards to boot it.
    async fn create(
        &self,
        params: &CreateMachineParams,
    ) -> Result<Machine, Box<dyn Error + Send + Sync>>;

    /// Destroy a machine and clean up all resources.
    ///
    /// Stops the machine if running, then removes everything.
    async fn destroy(&self, machine: &Machine) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Start a stopped machine. No-op if already running.
    async fn start(&self, machine: &Machine) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Stop a running machine.
    ///
    /// With `force: false`, sends a graceful shutdown signal first and falls
    /// back to force-kill if the guest doesn't respond.
    /// With `force: true`, kills immediately.
    async fn stop(
        &self,
        machine: &Machine,
        force: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// List all machines managed by this provider.
    async fn list(&self) -> Result<Vec<Machine>, Box<dyn Error + Send + Sync>>;

    /// Resolve the network endpoint for a machine, if available.
    ///
    /// Returns `Ok(None)` if the machine is not yet reachable (e.g., still booting).
    async fn resolve_endpoint(
        &self,
        _machine: &Machine,
    ) -> Result<Option<MachineEndpoint>, Box<dyn Error + Send + Sync>> {
        Ok(None)
    }
}
