//! Migrate capability module.

use crate::machine::Machine;
use async_trait::async_trait;
use std::error::Error;

/// Migrate capability: Move machines between hosts.
///
/// This capability is for future use and enables live migration of machines
/// from one physical host to another without stopping them. This is useful
/// for load balancing and maintenance scenarios.
///
/// TODO: This is currently not implemented in any provider but is defined
/// for future extensibility.
#[async_trait]
pub trait Migrate: Send + Sync {
    /// Migrate a machine to a different host.
    ///
    /// This should move the running machine to a different physical host
    /// with minimal downtime.
    async fn migrate(
        &self,
        machine: &Machine,
        target_host: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;
}
