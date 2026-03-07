//! Machinery manager for orchestrating machine allocation and provisioning
//!
//! This module provides a trait-based abstraction for managing machines across
//! different provider types. The actual implementation (pooled vs on-demand) is
//! selected at runtime based on provider capabilities.

use crate::error::{ResourceError, Result};
use crate::pool::{MachinePool, PoolConfig};
use crate::{Machine, MachineSpec, ProviderHandle};
use async_trait::async_trait;
use malbox_machinery::provider::capabilities::Allocate;
use malbox_machinery::provisioner::{ProvisionContext, ProvisionStatus, Provisioner};
use std::sync::Arc;
use tokio::time::Instant;
use tracing::{debug, info, warn};

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
pub struct PooledManager {
    pool: Arc<MachinePool>,
    provider: Arc<ProviderHandle>,
    provisioner: Option<Arc<dyn Provisioner>>,
    machinery_config: malbox_config::MachineryConfig,
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

        let pool = Arc::new(MachinePool::new(snapshot, allocate, pool_config));

        Ok(Self {
            pool,
            provider,
            provisioner,
            machinery_config: machinery_config.clone(),
        })
    }

    /// Initialize the pool by allocating, provisioning, and snapshotting machines.
    ///
    /// This creates the minimum number of machines as specified in the configuration.
    pub async fn initialize_pool(&self) -> Result<()> {
        info!("Initializing machine pool");

        let snapshot = self
            .provider
            .snapshot()
            .expect("Snapshot capability validated in new()");
        let allocate = self.provider.allocate();

        // Build machine spec from configuration
        let spec = MachineSpec {
            name: "pool-machine".to_string(),
            platform: crate::Platform::Linux, // TODO: Make configurable
            resources: crate::Resources {
                cpus: self.machinery_config.defaults.cpus,
                memory_mb: self.machinery_config.defaults.memory as u32,
            },
            storage: crate::Storage {
                boot_disk_gb: 64, // TODO: Make configurable
                disk_type: crate::DiskType::Qcow2,
            },
            network: crate::Network {
                mode: crate::NetworkMode::Nat,
                ip: None,
                mac: None,
            },
            base_image: self.machinery_config.defaults.base_image.clone(),
            provisioning: None,
        };

        let machines_needed = self.pool.machines_needed().await;
        for i in 0..machines_needed {
            info!("Initializing pool machine {}/{}", i + 1, machines_needed);

            // Allocate machine
            let mut machine = allocate
                .allocate(&spec)
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
                let endpoint = machine.endpoint().ok_or_else(|| {
                    ResourceError::MachineNotReady {
                        reason: "No endpoint available after readiness check".to_string(),
                    }
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
                    .map_err(|e| ResourceError::Provisioner(e.to_string()))?;

                if result.status == ProvisionStatus::Failed {
                    return Err(ResourceError::Provisioner(format!(
                        "Provisioner '{}' reported failure: {}",
                        provisioner.name(),
                        result.output.as_deref().unwrap_or("no output"),
                    )));
                }
            }

            // Create clean snapshot
            let clean_snapshot = snapshot
                .create_snapshot(&machine, &self.machinery_config.pool.clean_snapshot_name)
                .await
                .map_err(|e| ResourceError::Provider(e.to_string()))?;

            // Add to pool
            self.pool.add_machine(machine, clean_snapshot).await?;
        }

        info!("Pool initialized with {} machines", machines_needed);
        Ok(())
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
        Self { provider, provisioner }
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
            let endpoint = machine.endpoint().ok_or_else(|| {
                ResourceError::MachineNotReady {
                    reason: "No endpoint available after readiness check".to_string(),
                }
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

            info!("Provisioning on-demand machine with '{}'", provisioner.name());
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
