//! Native provisioner stub for Malbox.
//!
//! Placeholder for a future built-in provisioner that will handle
//! machine setup via SSH commands and file uploads directly.
//! Currently a no-op.

use async_trait::async_trait;
use malbox_machinery::provisioner::{
    ProvisionContext, ProvisionResult, ProvisionStatus, Provisioner, ProvisionerMetadata,
};
use std::error::Error;
use tracing::info;

/// Native provisioner (no-op stub).
pub struct NativeProvisioner;

impl NativeProvisioner {
    pub fn new(
        _config: &malbox_machinery::provisioner::TomlValue,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        Ok(Self)
    }
}

#[async_trait]
impl Provisioner for NativeProvisioner {
    async fn provision(
        &self,
        context: &ProvisionContext,
    ) -> Result<ProvisionResult, Box<dyn Error + Send + Sync>> {
        info!(
            "Native provisioner (no-op) for machine at {}",
            context.endpoint.address
        );

        Ok(ProvisionResult {
            status: ProvisionStatus::Success,
            output: None,
        })
    }

    fn name(&self) -> &str {
        "native"
    }
}

// Register with inventory
inventory::submit! {
    ProvisionerMetadata {
        name: "native",
        create: |config| {
            let provisioner = NativeProvisioner::new(config)?;
            Ok(Box::new(provisioner))
        },
    }
}
