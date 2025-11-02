use super::machine::{Machine, MachineId, MachineSpec};
use async_trait::async_trait;

/// Provider trait that all providers must implement.
#[async_trait]
pub trait Provider: Sized + Send + Sync + 'static {
    /// Provider-specific configuration.
    type Config: Send + Sync;
    /// Provider-specific machine type.
    type Machine: Machine;
    /// Provider-specific error type.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Provider capabilities.
    const CAPABILITIES: Capabilities;

    /// Initialize the provider from configuration.
    async fn initialize(config: Self::Config) -> Result<Self, Self::Error>;
    /// Allocate a machine.
    async fn allocate(&self, spec: MachineSpec) -> Result<Self::Machine, Self::Error>;
    /// List machines.
    async fn list(&self) -> Result<Vec<Self::Machine>, Self::Error>;
}

#[derive(Debug, Clone)]
pub struct Capabilities {}
