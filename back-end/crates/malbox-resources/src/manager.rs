//! Machinery manager for orchestrating machine allocation and provisioning
//!
//! This module provides a trait-based abstraction for managing machines across
//! different provider types. The actual implementation (pooled vs on-demand) is
//! selected at runtime based on provider capabilities.

use crate::error::{ResourceError, Result};
use crate::pool::{MachinePool, PoolConfig};
use crate::{Machine, MachineSpec, ProviderHandle};
use async_trait::async_trait;
use malbox_database::PgPool;
use malbox_database::repositories::machinery::{self, ProvisionStatusDb};
use malbox_machinery::provider::capabilities::Allocate;
use malbox_machinery::provisioner::{ProvisionContext, ProvisionStatus, Provisioner};
use std::sync::Arc;
use tokio::time::Instant;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Trait for machinery management operations.
///
/// This trait provides a unified interface for allocating and releasing machines,
/// regardless of whether the implementation uses pooling (for snapshot-capable
/// providers) or on-demand allocation.
#[async_trait]
pub trait MachineryManager: Send + Sync {
    /// Allocate a machine for task execution.
    ///
    /// For pooled managers, this acquires a machine from the pool and restores
    /// it to a clean snapshot. For on-demand managers, this allocates and
    /// provisions a new machine.
    async fn allocate(&self, spec: &MachineSpec) -> Result<Machine>;

    /// Release a machine after task execution.
    ///
    /// For pooled managers, this returns the machine to the pool. For on-demand
    /// managers, this deallocates the machine.
    async fn release(&self, machine: &Machine) -> Result<()>;

    /// Shutdown the manager and clean up resources.
    async fn shutdown(&self) -> Result<()>;
}

/// Pooled machinery manager for snapshot-capable providers.
///
/// This manager maintains a pool of pre-allocated and pre-provisioned machines.
/// Machines are quickly restored to a clean snapshot state between uses.
/// The pool state is persisted in the database for reconciliation on restart.
pub struct PooledManager {
    pool: Arc<MachinePool>,
    provider: Arc<ProviderHandle>,
    provisioner: Option<Arc<dyn Provisioner>>,
    machinery_config: malbox_config::MachineryConfig,
    db: PgPool,
}

impl PooledManager {
    /// Create a new pooled manager with empty pool.
    ///
    /// Call `initialize_pool()` to populate the pool with machines.
    ///
    /// # Errors
    ///
    /// Returns an error if the provider doesn't support the Snapshot capability.
    pub fn new(
        provider: Arc<ProviderHandle>,
        machinery_config: &malbox_config::MachineryConfig,
        provisioner: Option<Arc<dyn Provisioner>>,
        db: PgPool,
    ) -> Result<Self> {
        info!("Creating pooled machinery manager");

        // Validate that provider has Snapshot capability
        let snapshot = provider
            .snapshot()
            .ok_or_else(|| {
                ResourceError::Internal(format!(
                    "Provider '{}' does not support Snapshot capability",
                    provider.name()
                ))
            })?
            .clone();

        let allocate = provider.allocate().clone();

        // Convert config pool config to resource pool config
        let pool_config = PoolConfig {
            min_ready: machinery_config.pool.min_size,
            max_size: machinery_config.pool.max_size,
        };

        let pool = Arc::new(MachinePool::new(
            snapshot,
            allocate,
            pool_config,
            db.clone(),
        ));

        Ok(Self {
            pool,
            provider,
            provisioner,
            machinery_config: machinery_config.clone(),
            db,
        })
    }

    /// Initialize the pool with a two-phase approach:
    ///
    /// 1. **Reconcile** — Load existing pool machines from DB and validate
    ///    that they still exist in the provider with valid snapshots.
    /// 2. **Fill** — Provision new machines to reach the minimum pool size.
    pub async fn initialize_pool(&self, image_id: Uuid, image_path: &str) -> Result<()> {
        info!("Initializing machine pool (reconcile + fill)");

        let snapshot = self
            .provider
            .snapshot()
            .expect("Snapshot capability validated in new()");
        let allocate = self.provider.allocate();

        // Phase 1: Reconcile existing pool machines from DB
        let db_machines = self.pool.load_from_db().await?;
        if !db_machines.is_empty() {
            info!(
                "Reconciling {} existing pool machines from database",
                db_machines.len()
            );

            // Get list of VMs currently in the provider
            let provider_machines = allocate
                .list()
                .await
                .map_err(|e| ResourceError::Provider(e.to_string()))?;

            for db_machine in &db_machines {
                let db_id = match db_machine.id {
                    Some(id) => id,
                    None => {
                        warn!("DB machine missing id, skipping");
                        continue;
                    }
                };

                // Must have provision_status = Provisioned, clean_snapshot set, provider_id set
                let is_provisioned = db_machine.provision_status
                    == Some(ProvisionStatusDb::Provisioned);
                let clean_snap_name = match &db_machine.clean_snapshot {
                    Some(s) if !s.is_empty() => s.clone(),
                    _ => {
                        warn!(
                            "DB machine {} missing clean_snapshot, removing from pool",
                            db_id
                        );
                        let _ = machinery::remove_pool_machine(&self.db, db_id).await;
                        continue;
                    }
                };
                let provider_id = match &db_machine.provider_id {
                    Some(id) if !id.is_empty() => id.clone(),
                    _ => {
                        warn!(
                            "DB machine {} missing provider_id, removing from pool",
                            db_id
                        );
                        let _ = machinery::remove_pool_machine(&self.db, db_id).await;
                        continue;
                    }
                };

                if !is_provisioned {
                    warn!(
                        "DB machine {} not provisioned (status={:?}), removing from pool",
                        db_id, db_machine.provision_status
                    );
                    let _ = machinery::remove_pool_machine(&self.db, db_id).await;
                    continue;
                }

                // Check VM still exists in provider
                let runtime_machine = provider_machines
                    .iter()
                    .find(|m| m.id.0 == provider_id);

                let runtime_machine = match runtime_machine {
                    Some(m) => m.clone(),
                    None => {
                        warn!(
                            "DB machine {} (provider_id={}) not found in provider, removing from pool",
                            db_id, provider_id
                        );
                        let _ = machinery::remove_pool_machine(&self.db, db_id).await;
                        continue;
                    }
                };

                // Check snapshot still exists
                let snapshots = match snapshot.list_snapshots(&runtime_machine).await {
                    Ok(s) => s,
                    Err(e) => {
                        warn!(
                            "Failed to list snapshots for machine {} (provider_id={}): {}, removing from pool",
                            db_id, provider_id, e
                        );
                        let _ = machinery::remove_pool_machine(&self.db, db_id).await;
                        continue;
                    }
                };

                let snap_info = snapshots
                    .iter()
                    .find(|s| s.name == clean_snap_name);

                match snap_info {
                    Some(info) => {
                        info!(
                            "Reconciled machine {} (provider_id={}) with snapshot '{}'",
                            db_id, provider_id, clean_snap_name
                        );
                        self.pool
                            .add_machine(runtime_machine, info.id.clone(), db_id)
                            .await?;
                    }
                    None => {
                        warn!(
                            "Snapshot '{}' not found for machine {} (provider_id={}), removing from pool",
                            clean_snap_name, db_id, provider_id
                        );
                        let _ = machinery::remove_pool_machine(&self.db, db_id).await;
                    }
                }
            }
        }

        // Phase 2: Fill to minimum pool size
        let machines_needed = self.pool.machines_needed().await;
        if machines_needed == 0 {
            info!("Pool already at minimum size, no new machines needed");
            return Ok(());
        }

        info!(
            "Filling pool: provisioning {} new machines",
            machines_needed
        );

        let spec = self.build_spec(image_path);

        for i in 0..machines_needed {
            info!(
                "Provisioning pool machine {}/{}",
                i + 1,
                machines_needed
            );

            match self.provision_new_machine(&spec, image_id).await {
                Ok(_) => {}
                Err(e) => {
                    warn!("Failed to provision pool machine {}: {}", i + 1, e);
                    return Err(e);
                }
            }
        }

        let stats = self.pool.stats().await;
        info!(
            "Pool initialized: {} total, {} available",
            stats.total, stats.available
        );
        Ok(())
    }

    /// Provision a new machine and add it to the pool.
    ///
    /// Steps: allocate VM, wait for endpoint, insert into DB, provision,
    /// create snapshot, update DB status, add to in-memory pool.
    async fn provision_new_machine(
        &self,
        spec: &MachineSpec,
        image_id: Uuid,
    ) -> Result<()> {
        let snapshot_cap = self
            .provider
            .snapshot()
            .expect("Snapshot capability validated in new()");
        let allocate = self.provider.allocate();

        // 1. Allocate VM from provider
        let mut machine = allocate
            .allocate(spec)
            .await
            .map_err(|e| ResourceError::AllocationFailed(e.to_string()))?;

        // 2. Wait for endpoint
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

        // 3. Insert into DB as unprovisioned
        let db_machine = machinery::upsert_pool_machine(
            &self.db,
            &spec.name,
            &spec.name,
            machinery::MachinePlatform::Linux,  // TODO: Make configurable
            machinery::MachineArch::X64,        // TODO: Make configurable
            &ip_str,
            image_id,
            ProvisionStatusDb::Unprovisioned,
            None,
            self.provider.name(),
            &machine.id.0,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        let db_id = db_machine.id.unwrap_or(0);

        // 4. Update status to provisioning
        let _ = machinery::update_provision_status(
            &self.db,
            db_id,
            ProvisionStatusDb::Provisioning,
            None,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        // 5. Run provisioner if configured
        if let Some(ref provisioner) = self.provisioner {
            let endpoint =
                machine
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

            info!("Provisioning pool machine with '{}'", provisioner.name());
            let result = provisioner
                .provision(&context)
                .await
                .map_err(|e| ResourceError::Provisioner(e.to_string()));

            match result {
                Ok(result) if result.status == ProvisionStatus::Failed => {
                    // 6. On failure: update status to failed
                    let _ = machinery::update_provision_status(
                        &self.db,
                        db_id,
                        ProvisionStatusDb::Failed,
                        None,
                    )
                    .await;
                    return Err(ResourceError::Provisioner(format!(
                        "Provisioner '{}' reported failure: {}",
                        provisioner.name(),
                        result.output.as_deref().unwrap_or("no output"),
                    )));
                }
                Err(e) => {
                    let _ = machinery::update_provision_status(
                        &self.db,
                        db_id,
                        ProvisionStatusDb::Failed,
                        None,
                    )
                    .await;
                    return Err(e);
                }
                Ok(_) => {
                    // Success, continue
                }
            }
        }

        // 7. Create clean snapshot
        let clean_snapshot = snapshot_cap
            .create_snapshot(&machine, &self.machinery_config.pool.clean_snapshot_name)
            .await
            .map_err(|e| ResourceError::Provider(e.to_string()))?;

        // 8. Update status to provisioned with snapshot name
        let _ = machinery::update_provision_status(
            &self.db,
            db_id,
            ProvisionStatusDb::Provisioned,
            Some(&self.machinery_config.pool.clean_snapshot_name),
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        // 9. Add to in-memory pool
        self.pool.add_machine(machine, clean_snapshot, db_id).await?;

        Ok(())
    }

    /// Build a MachineSpec from the current config and image path.
    fn build_spec(&self, image_path: &str) -> MachineSpec {
        MachineSpec {
            name: "pool-machine".to_string(),
            platform: crate::Platform::Linux,
            resources: crate::Resources {
                cpus: self.machinery_config.defaults.cpus,
                memory_mb: self.machinery_config.defaults.memory as u32,
            },
            storage: crate::Storage {
                boot_disk_gb: 64,
                disk_type: crate::DiskType::Qcow2,
            },
            network: crate::Network {
                mode: crate::NetworkMode::Nat,
                ip: None,
                mac: None,
            },
            base_image: Some(image_path.to_string()),
            provisioning: None,
        }
    }
}

#[async_trait]
impl MachineryManager for PooledManager {
    async fn allocate(&self, _spec: &MachineSpec) -> Result<Machine> {
        debug!("Acquiring machine from pool");
        let machine = self.pool.acquire().await?;
        Ok(machine)
    }

    async fn release(&self, machine: &Machine) -> Result<()> {
        debug!("Releasing machine to pool");
        self.pool.release(&machine.id).await
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down pooled manager");
        self.pool.shutdown().await
    }
}

/// On-demand machinery manager for any provider.
///
/// This manager allocates and provisions machines on-demand for each task.
/// No pooling or snapshots are used.
pub struct OnDemandManager {
    provider: Arc<ProviderHandle>,
    provisioner: Option<Arc<dyn Provisioner>>,
}

impl OnDemandManager {
    /// Create a new on-demand manager.
    pub fn new(provider: Arc<ProviderHandle>, provisioner: Option<Arc<dyn Provisioner>>) -> Self {
        info!("Creating on-demand machinery manager");
        Self {
            provider,
            provisioner,
        }
    }
}

#[async_trait]
impl MachineryManager for OnDemandManager {
    async fn allocate(&self, spec: &MachineSpec) -> Result<Machine> {
        debug!("Allocating machine on-demand");

        let allocate = self.provider.allocate();

        // Allocate from provider
        let mut machine = allocate
            .allocate(spec)
            .await
            .map_err(|e| ResourceError::AllocationFailed(e.to_string()))?;

        // Wait for machine to be ready (poll for endpoint)
        wait_for_endpoint(
            allocate.as_ref(),
            &mut machine,
            std::time::Duration::from_secs(120),
        )
        .await?;

        // Provision if configured
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
                "Provisioning on-demand machine with '{}'",
                provisioner.name()
            );
            let result = provisioner
                .provision(&context)
                .await
                .map_err(|e| ResourceError::Provisioner(e.to_string()))?;

            if result.status == ProvisionStatus::Failed {
                return Err(ResourceError::Provisioner(format!(
                    "Provisioner '{}' reported failure: {}",
                    provisioner.name(),
                    result.output.as_deref().unwrap_or("no output"),
                )));
            }
        }

        Ok(machine)
    }

    async fn release(&self, machine: &Machine) -> Result<()> {
        debug!("Deallocating on-demand machine");

        let allocate = self.provider.allocate();

        // Deallocate through the provider
        allocate
            .deallocate(machine)
            .await
            .map_err(|e| ResourceError::DeallocationFailed(e.to_string()))?;

        info!("Machine {:?} deallocated", machine.id);
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down on-demand manager");
        Ok(())
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
