//! Provisioner trait and registry.
//!
//! Provisioners handle post-allocation machine setup (installing packages,
//! running scripts, applying configurations). They are transport-agnostic —
//! each provisioner manages its own connectivity to the machine.
//!
//! Registration uses the `inventory` crate, same pattern as providers.

use crate::machine::MachineEndpoint;
use async_trait::async_trait;
use std::error::Error;

pub mod config;

// Re-export toml::Value so provisioners don't need to depend on toml directly.
pub use toml::Value as TomlValue;

/// Trait for machine provisioners.
///
/// Provisioners receive a [`ProvisionContext`] with the machine's network
/// endpoint and provisioner-specific configuration. They handle their own
/// connectivity (WinRM, SSH, etc.).
#[async_trait]
pub trait Provisioner: Send + Sync {
    /// Provision a machine.
    ///
    /// Called once the machine is running and reachable. The provisioner
    /// should perform all setup steps and return when complete.
    async fn provision(
        &self,
        context: &ProvisionContext,
    ) -> Result<ProvisionResult, Box<dyn Error + Send + Sync>>;

    /// Human-readable name for logging.
    fn name(&self) -> &str;
}

/// Context passed to a provisioner during provisioning.
#[derive(Debug, Clone)]
pub struct ProvisionContext {
    /// Network endpoint of the machine (IP, id, platform).
    pub endpoint: MachineEndpoint,
    /// Provisioner-specific configuration from the step's TOML config.
    pub config: toml::Value,
}

/// Result of a provisioning operation.
#[derive(Debug, Clone)]
pub struct ProvisionResult {
    /// Whether provisioning succeeded or failed.
    pub status: ProvisionStatus,
    /// Optional output/logs from the provisioner.
    pub output: Option<String>,
}

/// Status of a provisioning operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionStatus {
    Success,
    Failed,
}

/// Factory function signature used by a registered provisioner.
pub type ProvisionerFactory =
    fn(&toml::Value) -> Result<Box<dyn Provisioner>, Box<dyn Error + Send + Sync>>;

/// Metadata about a registered provisioner.
///
/// Submitted to the `inventory` registry by provisioner crates.
pub struct ProvisionerMetadata {
    /// Unique name for this provisioner (e.g., "ansible").
    pub name: &'static str,
    /// Factory function to create a provisioner instance from TOML config.
    pub create: ProvisionerFactory,
}

// Collect all registered provisioners at compile time.
inventory::collect!(ProvisionerMetadata);

/// Get provisioner metadata by name.
pub fn get_provisioner_metadata(name: &str) -> Option<&'static ProvisionerMetadata> {
    inventory::iter::<ProvisionerMetadata>().find(|p| p.name == name)
}

/// List all registered provisioner names.
pub fn list_provisioners() -> Vec<&'static str> {
    inventory::iter::<ProvisionerMetadata>()
        .map(|p| p.name)
        .collect()
}

/// Create a provisioner by name with configuration.
pub fn create_provisioner(
    name: &str,
    config: &toml::Value,
) -> Result<Box<dyn Provisioner>, Box<dyn Error + Send + Sync>> {
    let metadata = get_provisioner_metadata(name).ok_or_else(|| {
        format!(
            "Provisioner '{}' not found. Available provisioners: {:?}",
            name,
            list_provisioners()
        )
    })?;

    (metadata.create)(config)
}
