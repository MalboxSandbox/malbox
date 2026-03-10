//! Utility functions for machine lifecycle operations.

use crate::error::{ResourceError, Result};
use malbox_machinery::provider::capabilities::Allocate;
use malbox_machinery::Machine;
use tokio::time::Instant;
use tracing::{info, warn};

/// Wait until a machine has an endpoint (IP address) available.
///
/// Polls the provider's `resolve_endpoint` method until an endpoint is found
/// or the timeout expires. Uses exponential backoff starting at 2s, capped at 10s.
pub async fn wait_for_endpoint(
    allocate: &dyn Allocate,
    machine: &mut Machine,
    timeout: std::time::Duration,
) -> Result<()> {
    if machine.endpoint().is_some() {
        return Ok(());
    }

    let start = Instant::now();
    let mut interval = std::time::Duration::from_secs(2);
    let max_interval = std::time::Duration::from_secs(10);

    info!(
        machine_id = %machine.id(),
        timeout_secs = timeout.as_secs(),
        "Waiting for machine endpoint"
    );

    loop {
        tokio::time::sleep(interval).await;

        match allocate.resolve_endpoint(machine).await {
            Ok(Some(endpoint)) => {
                info!(
                    machine_id = %machine.id(),
                    address = %endpoint.address,
                    elapsed_secs = start.elapsed().as_secs(),
                    "Machine endpoint resolved"
                );
                machine.set_endpoint(Some(endpoint));
                return Ok(());
            }
            Ok(None) => {
                // Not ready yet, continue polling
            }
            Err(e) => {
                warn!(
                    machine_id = %machine.id(),
                    error = %e,
                    "Error resolving endpoint, will retry"
                );
            }
        }

        if start.elapsed() >= timeout {
            return Err(ResourceError::Timeout(format!(
                "Machine {} did not obtain an endpoint within {}s",
                machine.id(),
                timeout.as_secs(),
            )));
        }

        // Exponential backoff capped at max_interval
        interval = (interval * 2).min(max_interval);
    }
}
