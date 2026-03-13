//! Provisioning: allocate VM, wait for endpoint, run provisioner, snapshot, mark ready.

use crate::error::{ResourceError, Result};
use malbox_database::repositories::machinery::{self, MachineStatusDb};
use malbox_machinery::Machine;
use malbox_machinery::provider::capabilities::Allocate;
use malbox_machinery::provisioner::{ProvisionContext, ProvisionStatus};
use tokio::time::Instant;
use tracing::{info, warn};

use super::MachinePool;

impl MachinePool {
    /// Full provisioning pipeline for a machine.
    ///
    /// 1. Allocate VM from provider
    /// 2. Wait for endpoint (IP)
    /// 3. Update provider info in DB
    /// 4. Run provisioner (if configured)
    /// 5. Create clean snapshot (if provider supports snapshots)
    /// 6. Mark ready
    pub(crate) async fn provision_machine(&self, machine_id: i32, image_path: &str) -> Result<()> {
        let allocate = self.provider.allocate();

        machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Provisioning, None)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        let spec = self.build_spec(machine_id, image_path);
        let mut machine = allocate
            .allocate(&spec)
            .await
            .map_err(|e| ResourceError::AllocationFailed(e.to_string()))?;

        wait_for_endpoint(
            allocate.as_ref(),
            &mut machine,
            std::time::Duration::from_secs(120),
        )
        .await?;

        let ip_str = machine
            .endpoint()
            .map(|ep| ep.address.to_string())
            .unwrap_or_default();

        machinery::set_machine_provider_info(
            &self.db,
            machine_id,
            self.provider.name(),
            &machine.id.0,
            &ip_str,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        if let Some(ref provisioner) = self.provisioner {
            let endpoint = machine
                .endpoint()
                .ok_or_else(|| ResourceError::MachineNotReady {
                    reason: "No endpoint available after readiness check".to_string(),
                })?;

            let context = ProvisionContext {
                endpoint: endpoint.clone(),
                spec: spec.clone(),
                config: spec
                    .provisioning
                    .as_ref()
                    .map(|p| p.config.clone())
                    .unwrap_or(toml::Value::Table(toml::map::Map::new())),
            };

            info!(
                machine_id,
                provisioner = provisioner.name(),
                "Running provisioner"
            );
            let result = provisioner
                .provision(&context)
                .await
                .map_err(|e| ResourceError::Provisioner(e.to_string()))?;

            machinery::set_machine_provision_info(
                &self.db,
                machine_id,
                provisioner.name(),
                result.output.as_deref(),
            )
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

            if result.status == ProvisionStatus::Failed {
                return Err(ResourceError::Provisioner(format!(
                    "Provisioner '{}' reported failure: {}",
                    provisioner.name(),
                    result.output.as_deref().unwrap_or("no output"),
                )));
            }
        }

        if let Some(snapshot_cap) = self.provider.snapshot() {
            let snap_id = snapshot_cap
                .create_snapshot(&machine, &self.config.clean_snapshot_name)
                .await
                .map_err(|e| ResourceError::Provider(e.to_string()))?;

            machinery::set_machine_snapshot(&self.db, machine_id, &self.config.clean_snapshot_name)
                .await
                .map_err(|e| ResourceError::Database(e.to_string()))?;

            info!(machine_id, snapshot_id = %snap_id, "Clean snapshot created");
        }

        machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Ready, None)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        self.machine_available.notify_waiters();
        info!(machine_id, "Machine is ready");

        Ok(())
    }

    fn build_spec(&self, machine_id: i32, image_path: &str) -> malbox_machinery::MachineSpec {
        malbox_machinery::MachineSpec {
            name: format!("malbox-machine-{}", machine_id),
            platform: malbox_machinery::Platform::Linux, // TODO: map from DB
            resources: malbox_machinery::Resources {
                cpus: self.config.defaults.cpus,
                memory_mb: self.config.defaults.memory as u32,
            },
            storage: malbox_machinery::Storage {
                boot_disk_gb: 64,
                disk_type: malbox_machinery::DiskType::Qcow2,
            },
            network: malbox_machinery::Network {
                mode: malbox_machinery::NetworkMode::Nat,
                ip: None,
                mac: None,
            },
            base_image: Some(image_path.to_string()),
            provisioning: None,
        }
    }
}

/// Wait until a machine has an endpoint (IP address) available.
///
/// Polls the provider's `resolve_endpoint` method until an endpoint is found
/// or the timeout expires. Uses exponential backoff starting at 2s, capped at 10s.
async fn wait_for_endpoint(
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
            Ok(None) => {}
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

        interval = (interval * 2).min(max_interval);
    }
}
