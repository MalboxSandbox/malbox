//! Allocate capability — core machine lifecycle management.

use crate::machine::{CreateMachineParams, Machine, MachineEndpoint, ManagementAccess, Platform};
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
    /// With [`StopMode::Graceful`], sends a graceful shutdown signal first and
    /// falls back to force-kill if the guest doesn't respond.
    /// With [`StopMode::Force`], kills immediately.
    async fn stop(
        &self,
        machine: &Machine,
        mode: StopMode,
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

    /// How provisioners reach this machine's management interface.
    ///
    /// Defaults to the platform's network management port (`WinRM` 5986 on
    /// Windows, SSH 22 on Linux). Providers without a guest network service
    /// override this.
    fn management_access(&self, _machine: &Machine, platform: Platform) -> ManagementAccess {
        match platform {
            Platform::Windows => ManagementAccess::Network { port: 5986 },
            Platform::Linux => ManagementAccess::Network { port: 22 },
        }
    }
}

/// How [`Allocate::stop`] shuts a machine down.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StopMode {
    /// Request a guest shutdown, force-killing if it does not comply.
    Graceful,
    /// Kill immediately without notifying the guest.
    Force,
}
