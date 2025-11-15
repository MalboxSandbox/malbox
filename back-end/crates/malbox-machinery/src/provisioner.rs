use async_trait::async_trait;
use std::error::Error;

use crate::machine::{MachineEndpoint, MachineSpec};

/// Provisioner trait for configuring machines after allocation.
#[async_trait]
pub trait Provisioner: Send + Sync {
    type Error: Error + Send + Sync + 'static;

    /// Provision a machine using its specification and network endpoint.
    async fn provision(
        &self,
        spec: &MachineSpec,
        endpoint: &MachineEndpoint,
    ) -> Result<(), Self::Error>;
}
