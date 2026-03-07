//! Clone capability module.

use crate::machine::Machine;
use async_trait::async_trait;
use std::error::Error;

/// Clone capability: Create copies of machines.
///
/// Providers that implement this capability can create independent copies
/// of existing machines. This is useful for scaling out workloads and
/// streaming strategies that need multiple instances of the same configuration.
#[async_trait]
pub trait Clone: Send + Sync {
    /// Clone an existing machine into a new independent machine.
    ///
    /// The cloned machine should be an exact copy of the original, but with
    /// a new name and independent lifecycle. Changes to one machine should
    /// not affect the other.
    async fn clone_machine(
        &self,
        machine: &Machine,
        new_name: &str,
    ) -> Result<Machine, Box<dyn Error + Send + Sync>>;
}
