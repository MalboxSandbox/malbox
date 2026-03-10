//! Machine pool for managing persistent, user-created machines.
//!
//! `MachinePool` is the single entry point for machine lifecycle management.
//! It replaces the old `PooledManager` / `OnDemandManager` split with a unified
//! struct that owns a provider, an optional provisioner, and a database handle.
//!
//! Machines progress through the following states:
//!   creating -> provisioning -> ready -> assigned -> reverting -> ready
//!                                                                 \-> failed
//!                                                    deleting -> (removed)

use crate::error::{ResourceError, Result};
use crate::manager::wait_for_endpoint;
use malbox_config::machinery::MachineDefaults;
use malbox_database::repositories::images;
use malbox_database::repositories::machinery::{
    self, MachineArch, MachinePlatform, MachineStatusDb,
};
use malbox_database::PgPool;
use malbox_machinery::provisioner::{ProvisionContext, ProvisionStatus, Provisioner};
use malbox_machinery::ProviderHandle;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{error, info, warn};

// Re-export the DB machine type under a short alias used by callers.
pub type DbMachine = machinery::Machine;

/// Configuration extracted from `MachineryConfig` for pool use.
#[derive(Debug, Clone)]
pub struct MachinePoolConfig {
    pub clean_snapshot_name: String,
    pub defaults: MachineDefaults,
}

/// Request payload for creating a new machine.
#[derive(Debug, Clone)]
pub struct CreateMachineRequest {
    pub name: String,
    pub image: String,
    pub platform: MachinePlatform,
    pub arch: MachineArch,
    pub cpus: Option<u32>,
    pub memory_mb: Option<u64>,
}

/// Central machine pool that owns all lifecycle operations.
///
/// All heavy work (provisioning, reverting) is spawned into background tokio
/// tasks so that API calls return immediately.  The `machine_available`
/// [`Notify`] is signalled whenever a machine transitions to `ready`.
pub struct MachinePool {
    provider: Arc<ProviderHandle>,
    provisioner: Option<Arc<dyn Provisioner>>,
    db: PgPool,
    config: MachinePoolConfig,
    machine_available: Arc<Notify>,
}

impl MachinePool {
    /// Create a new `MachinePool`.
    pub fn new(
        provider: Arc<ProviderHandle>,
        provisioner: Option<Arc<dyn Provisioner>>,
        db: PgPool,
        config: MachinePoolConfig,
    ) -> Self {
        Self {
            provider,
            provisioner,
            db,
            config,
            machine_available: Arc::new(Notify::new()),
        }
    }

    /// Return a handle to the [`Notify`] that fires when a machine becomes ready.
    pub fn available_notifier(&self) -> Arc<Notify> {
        self.machine_available.clone()
    }

    /// Clone all `Arc` references so the pool can be moved into a `tokio::spawn`.
    pub fn clone_for_spawn(&self) -> MachinePool {
        MachinePool {
            provider: self.provider.clone(),
            provisioner: self.provisioner.clone(),
            db: self.db.clone(),
            config: self.config.clone(),
            machine_available: self.machine_available.clone(),
        }
    }

    // ------------------------------------------------------------------
    // Public API
    // ------------------------------------------------------------------

    /// Create a new machine.
    ///
    /// Validates that the requested image exists, inserts a DB row in
    /// `creating` status, and spawns background provisioning.  Returns the
    /// DB row immediately so the caller can track progress.
    pub async fn create_machine(&self, request: CreateMachineRequest) -> Result<DbMachine> {
        // 1. Validate image exists
        let image = images::fetch_image_by_name(&self.db, &request.image)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or_else(|| ResourceError::Internal(format!(
                "Image '{}' not found",
                request.image,
            )))?;

        // 2. Insert DB row in 'creating' status
        let db_machine = machinery::insert_machine(
            &self.db,
            &request.name,
            request.platform,
            request.arch,
            image.id,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        let machine_id = db_machine.id.expect("insert_machine always returns id");
        let image_path = image.path.clone();

        // 3. Spawn background provisioning
        let pool = self.clone_for_spawn();
        tokio::spawn(async move {
            if let Err(e) = pool.provision_machine(machine_id, &image_path).await {
                error!(machine_id, error = %e, "Background provisioning failed");
                let _ = machinery::update_machine_status(
                    &pool.db,
                    machine_id,
                    MachineStatusDb::Failed,
                    Some(&e.to_string()),
                )
                .await;
            }
        });

        Ok(db_machine)
    }

    /// Acquire a ready machine for a task.
    ///
    /// Delegates to the database's atomic `acquire_machine` query which uses
    /// `FOR UPDATE SKIP LOCKED` to avoid contention.
    pub async fn acquire(
        &self,
        platform: MachinePlatform,
        task_id: i32,
    ) -> Result<Option<DbMachine>> {
        machinery::acquire_machine(&self.db, platform, task_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))
    }

    /// Release a machine after task completion.
    ///
    /// Marks the machine as `reverting` and spawns an async revert.
    pub async fn release(&self, machine_id: i32) -> Result<()> {
        // Mark reverting in DB (also clears current_task_id)
        machinery::release_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        // Spawn background revert
        let pool = self.clone_for_spawn();
        tokio::spawn(async move {
            if let Err(e) = pool.revert_machine(machine_id).await {
                error!(machine_id, error = %e, "Background revert failed");
                let _ = machinery::update_machine_status(
                    &pool.db,
                    machine_id,
                    MachineStatusDb::Failed,
                    Some(&e.to_string()),
                )
                .await;
            }
        });

        Ok(())
    }

    /// Delete a machine.
    ///
    /// Rejects the request if the machine is currently assigned.  Otherwise,
    /// marks `deleting`, deallocates from the provider, and removes the DB row.
    pub async fn delete_machine(&self, machine_id: i32) -> Result<()> {
        let db_machine = self.require_machine(machine_id).await?;

        if db_machine.status == MachineStatusDb::Assigned {
            return Err(ResourceError::MachineAssigned { id: machine_id });
        }

        // Mark deleting
        machinery::update_machine_status(
            &self.db,
            machine_id,
            MachineStatusDb::Deleting,
            None,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        // Deallocate from provider if we have a provider_id
        if let Some(ref provider_id) = db_machine.provider_id {
            if let Err(e) = self.deallocate_by_provider_id(provider_id).await {
                warn!(machine_id, error = %e, "Failed to deallocate from provider during delete (continuing)");
            }
        }

        // Remove from DB
        machinery::delete_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        info!(machine_id, "Machine deleted");
        Ok(())
    }

    /// Retry provisioning of a failed machine.
    ///
    /// Only valid for machines in `failed` status.  Resets to `creating` and
    /// re-runs the provisioning pipeline.
    pub async fn retry(&self, machine_id: i32) -> Result<DbMachine> {
        let db_machine = self.require_machine(machine_id).await?;

        if db_machine.status != MachineStatusDb::Failed {
            return Err(ResourceError::InvalidMachineState {
                id: machine_id,
                status: format!("{:?}", db_machine.status),
                expected: "failed".to_string(),
            });
        }

        // Reset to creating
        let updated = machinery::update_machine_status(
            &self.db,
            machine_id,
            MachineStatusDb::Creating,
            None, // clear error message
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        // Look up the image path
        let image_path = self.resolve_image_path(&db_machine).await?;

        // Spawn background provisioning
        let pool = self.clone_for_spawn();
        tokio::spawn(async move {
            if let Err(e) = pool.provision_machine(machine_id, &image_path).await {
                error!(machine_id, error = %e, "Retry provisioning failed");
                let _ = machinery::update_machine_status(
                    &pool.db,
                    machine_id,
                    MachineStatusDb::Failed,
                    Some(&e.to_string()),
                )
                .await;
            }
        });

        Ok(updated)
    }

    /// Reconcile DB state against the provider on startup.
    ///
    /// Validates that every machine in the database still exists in the
    /// provider and is in a consistent state.  Interrupted operations
    /// (creating, provisioning, reverting) are marked failed.
    pub async fn reconcile(&self) -> Result<()> {
        info!("Reconciling machine pool with provider");

        let db_machines = machinery::fetch_all_machines(&self.db)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        if db_machines.is_empty() {
            info!("No machines in database, nothing to reconcile");
            return Ok(());
        }

        let allocate = self.provider.allocate();
        let provider_vms = allocate
            .list()
            .await
            .map_err(|e| ResourceError::Provider(e.to_string()))?;

        for db_machine in &db_machines {
            let machine_id = match db_machine.id {
                Some(id) => id,
                None => continue,
            };

            match db_machine.status {
                // Interrupted operations -> failed
                MachineStatusDb::Creating | MachineStatusDb::Provisioning => {
                    warn!(machine_id, status = ?db_machine.status, "Interrupted operation, marking failed");
                    let _ = machinery::update_machine_status(
                        &self.db,
                        machine_id,
                        MachineStatusDb::Failed,
                        Some("Interrupted by restart"),
                    )
                    .await;
                }

                // Assigned or Reverting -> attempt snapshot revert, or mark failed
                MachineStatusDb::Assigned | MachineStatusDb::Reverting => {
                    warn!(machine_id, status = ?db_machine.status, "Machine was in-flight, attempting recovery");

                    // Clear task assignment
                    let _ = machinery::release_machine(&self.db, machine_id).await;

                    // Try to revert via snapshot
                    if self.try_snapshot_revert(machine_id, db_machine, &provider_vms).await {
                        info!(machine_id, "Recovered machine via snapshot revert");
                    } else {
                        warn!(machine_id, "Could not recover, marking failed");
                        let _ = machinery::update_machine_status(
                            &self.db,
                            machine_id,
                            MachineStatusDb::Failed,
                            Some("Could not recover after restart"),
                        )
                        .await;
                    }
                }

                // Ready -> verify the VM and snapshot still exist in provider
                MachineStatusDb::Ready => {
                    if let Some(ref provider_id) = db_machine.provider_id {
                        let vm_exists = provider_vms.iter().any(|vm| vm.id.0 == *provider_id);
                        if !vm_exists {
                            warn!(machine_id, provider_id, "Ready machine not found in provider, marking failed");
                            let _ = machinery::update_machine_status(
                                &self.db,
                                machine_id,
                                MachineStatusDb::Failed,
                                Some("VM not found in provider after restart"),
                            )
                            .await;
                            continue;
                        }

                        // Verify snapshot exists if provider has snapshot capability
                        if let Some(snapshot_cap) = self.provider.snapshot() {
                            if let Some(ref snap_name) = db_machine.clean_snapshot {
                                let runtime_vm = provider_vms
                                    .iter()
                                    .find(|vm| vm.id.0 == *provider_id);
                                if let Some(vm) = runtime_vm {
                                    match snapshot_cap.list_snapshots(vm).await {
                                        Ok(snaps) => {
                                            let snap_exists = snaps.iter().any(|s| s.name == *snap_name);
                                            if !snap_exists {
                                                warn!(machine_id, snap_name, "Snapshot not found, marking failed");
                                                let _ = machinery::update_machine_status(
                                                    &self.db,
                                                    machine_id,
                                                    MachineStatusDb::Failed,
                                                    Some("Clean snapshot not found in provider"),
                                                )
                                                .await;
                                            }
                                        }
                                        Err(e) => {
                                            warn!(machine_id, error = %e, "Failed to list snapshots, marking failed");
                                            let _ = machinery::update_machine_status(
                                                &self.db,
                                                machine_id,
                                                MachineStatusDb::Failed,
                                                Some(&format!("Failed to verify snapshot: {e}")),
                                            )
                                            .await;
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        warn!(machine_id, "Ready machine missing provider_id, marking failed");
                        let _ = machinery::update_machine_status(
                            &self.db,
                            machine_id,
                            MachineStatusDb::Failed,
                            Some("Missing provider_id"),
                        )
                        .await;
                    }
                }

                // Deleting -> finish deletion
                MachineStatusDb::Deleting => {
                    warn!(machine_id, "Machine was mid-deletion, finishing cleanup");
                    if let Some(ref provider_id) = db_machine.provider_id {
                        let _ = self.deallocate_by_provider_id(provider_id).await;
                    }
                    let _ = machinery::delete_machine(&self.db, machine_id).await;
                }

                // Failed -> leave as-is
                MachineStatusDb::Failed => {
                    info!(machine_id, "Machine already failed, skipping");
                }
            }
        }

        info!("Reconciliation complete");
        Ok(())
    }

    /// Fetch a single machine by ID.
    pub async fn get(&self, machine_id: i32) -> Result<Option<DbMachine>> {
        machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))
    }

    /// List all machines.
    pub async fn list(&self) -> Result<Vec<DbMachine>> {
        machinery::fetch_all_machines(&self.db)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Full provisioning pipeline for a machine.
    ///
    /// 1. Allocate VM from provider
    /// 2. Wait for endpoint (IP)
    /// 3. Update provider info in DB
    /// 4. Run provisioner (if configured)
    /// 5. Create clean snapshot (if provider supports snapshots)
    /// 6. Mark ready
    async fn provision_machine(&self, machine_id: i32, image_path: &str) -> Result<()> {
        let allocate = self.provider.allocate();

        // Update status to provisioning
        machinery::update_machine_status(
            &self.db,
            machine_id,
            MachineStatusDb::Provisioning,
            None,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        // 1. Build spec and allocate VM
        let spec = self.build_spec(machine_id, image_path);
        let mut machine = allocate
            .allocate(&spec)
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

        // 3. Update provider info in DB
        machinery::set_machine_provider_info(
            &self.db,
            machine_id,
            self.provider.name(),
            &machine.id.0,
            &ip_str,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        // 4. Run provisioner if configured
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

            info!(machine_id, provisioner = provisioner.name(), "Running provisioner");
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

        // 5. Create clean snapshot if provider supports it
        if let Some(snapshot_cap) = self.provider.snapshot() {
            let snap_id = snapshot_cap
                .create_snapshot(&machine, &self.config.clean_snapshot_name)
                .await
                .map_err(|e| ResourceError::Provider(e.to_string()))?;

            machinery::set_machine_snapshot(
                &self.db,
                machine_id,
                &self.config.clean_snapshot_name,
            )
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

            info!(machine_id, snapshot_id = %snap_id, "Clean snapshot created");
        }

        // 6. Mark ready
        machinery::update_machine_status(
            &self.db,
            machine_id,
            MachineStatusDb::Ready,
            None,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        self.machine_available.notify_waiters();
        info!(machine_id, "Machine is ready");

        Ok(())
    }

    /// Revert a machine to its clean state after task completion.
    ///
    /// If the provider supports snapshots and the machine has a recorded
    /// snapshot, restores it.  Otherwise, deallocates and re-provisions
    /// from the original image.
    async fn revert_machine(&self, machine_id: i32) -> Result<()> {
        let db_machine = self.require_machine(machine_id).await?;

        // Try snapshot-based revert first
        if let (Some(snapshot_cap), Some(snap_name), Some(provider_id)) = (
            self.provider.snapshot(),
            db_machine.clean_snapshot.as_deref(),
            db_machine.provider_id.as_deref(),
        ) {
            // Find the runtime machine from the provider
            let allocate = self.provider.allocate();
            let provider_vms = allocate
                .list()
                .await
                .map_err(|e| ResourceError::Provider(e.to_string()))?;

            if let Some(runtime_vm) = provider_vms.iter().find(|vm| vm.id.0 == provider_id) {
                // Find the snapshot by name
                let snapshots = snapshot_cap
                    .list_snapshots(runtime_vm)
                    .await
                    .map_err(|e| ResourceError::Provider(e.to_string()))?;

                if let Some(snap_info) = snapshots.iter().find(|s| s.name == *snap_name) {
                    // Restore snapshot
                    snapshot_cap
                        .restore_snapshot(runtime_vm, &snap_info.id)
                        .await
                        .map_err(|e| ResourceError::SnapshotRestoreFailed {
                            id: machine_id,
                            reason: e.to_string(),
                        })?;

                    // Mark ready
                    machinery::update_machine_status(
                        &self.db,
                        machine_id,
                        MachineStatusDb::Ready,
                        None,
                    )
                    .await
                    .map_err(|e| ResourceError::Database(e.to_string()))?;

                    self.machine_available.notify_waiters();
                    info!(machine_id, "Machine reverted via snapshot");
                    return Ok(());
                }
            }
        }

        // Fallback: deallocate and re-provision from image
        info!(machine_id, "No snapshot available, re-provisioning from image");

        // Deallocate existing VM if possible
        if let Some(ref provider_id) = db_machine.provider_id {
            let _ = self.deallocate_by_provider_id(provider_id).await;
        }

        // Resolve image path and re-provision
        let image_path = self.resolve_image_path(&db_machine).await?;

        // Reset to creating and re-provision
        machinery::update_machine_status(
            &self.db,
            machine_id,
            MachineStatusDb::Creating,
            None,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        self.provision_machine(machine_id, &image_path).await
    }

    /// Build a `MachineSpec` for provider allocation.
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

    /// Fetch a machine from DB, returning an error if not found.
    async fn require_machine(&self, machine_id: i32) -> Result<DbMachine> {
        machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or_else(|| ResourceError::MachineNotFound {
                id: machine_id.to_string(),
            })
    }

    /// Resolve the image path for a machine from its `image_id`.
    async fn resolve_image_path(&self, db_machine: &DbMachine) -> Result<String> {
        let image_id = db_machine.image_id.ok_or_else(|| {
            ResourceError::Internal("Machine has no image_id".to_string())
        })?;

        let image = images::fetch_image_by_id(&self.db, image_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or_else(|| ResourceError::Internal(format!(
                "Image with id {} not found",
                image_id,
            )))?;

        Ok(image.path)
    }

    /// Deallocate a VM by its provider-side ID.
    async fn deallocate_by_provider_id(&self, provider_id: &str) -> Result<()> {
        let allocate = self.provider.allocate();
        let provider_vms = allocate
            .list()
            .await
            .map_err(|e| ResourceError::Provider(e.to_string()))?;

        if let Some(vm) = provider_vms.iter().find(|vm| vm.id.0 == provider_id) {
            allocate
                .deallocate(vm)
                .await
                .map_err(|e| ResourceError::DeallocationFailed(e.to_string()))?;
        }

        Ok(())
    }

    /// Attempt a snapshot-based revert during reconciliation.
    ///
    /// Returns `true` if the revert succeeded and the machine was marked ready.
    async fn try_snapshot_revert(
        &self,
        machine_id: i32,
        db_machine: &DbMachine,
        provider_vms: &[malbox_machinery::Machine],
    ) -> bool {
        let snapshot_cap = match self.provider.snapshot() {
            Some(cap) => cap,
            None => return false,
        };

        let snap_name = match &db_machine.clean_snapshot {
            Some(name) if !name.is_empty() => name,
            _ => return false,
        };

        let provider_id = match &db_machine.provider_id {
            Some(id) if !id.is_empty() => id,
            _ => return false,
        };

        let runtime_vm = match provider_vms.iter().find(|vm| vm.id.0 == *provider_id) {
            Some(vm) => vm,
            None => return false,
        };

        let snapshots = match snapshot_cap.list_snapshots(runtime_vm).await {
            Ok(s) => s,
            Err(_) => return false,
        };

        let snap_info = match snapshots.iter().find(|s| s.name == *snap_name) {
            Some(info) => info,
            None => return false,
        };

        if snapshot_cap
            .restore_snapshot(runtime_vm, &snap_info.id)
            .await
            .is_err()
        {
            return false;
        }

        if machinery::update_machine_status(
            &self.db,
            machine_id,
            MachineStatusDb::Ready,
            None,
        )
        .await
        .is_ok()
        {
            self.machine_available.notify_waiters();
            true
        } else {
            false
        }
    }
}
