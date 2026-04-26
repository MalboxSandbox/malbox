//! Startup reconciliation — validate DB state against the provider.

use crate::error::{ResourceError, Result};
use malbox_config::MachineConfig;
use malbox_database::repositories::images;
use malbox_database::repositories::machinery::{self, MachineArch, MachineStatusDb};
use malbox_database::repositories::provision_runs;
use malbox_database::repositories::snapshots;
use malbox_machinery::provider::capabilities::snapshot::SnapshotId;
use tracing::{debug, debug_span, error, info, instrument, warn};

use super::provisioning::hash_provider_config;
use super::revert::destroy_by_provider_id;
use super::{DbMachine, MachinePool};

impl MachinePool {
    /// Reconcile DB state against the provider on startup.
    #[instrument(skip_all, fields(machine_count = tracing::field::Empty), err)]
    pub async fn reconcile(&self) -> Result<()> {
        info!("Reconciling machine pool with provider");

        // Mark any provision runs stuck in 'running' as failed
        match provision_runs::mark_interrupted_runs(&self.db).await {
            Ok(count) if count > 0 => {
                info!(count, "Marked interrupted provision runs as failed");
            }
            Err(e) => {
                warn!(error = %e, "Failed to clean up interrupted provision runs");
            }
            _ => {}
        }

        let db_machines = machinery::fetch_all_machines(&self.db)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        tracing::Span::current().record("machine_count", db_machines.len());

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
            let _span = debug_span!("reconcile.machine", machine_id).entered();

            match db_machine.status {
                MachineStatusDb::Creating => {
                    warn!(machine_id, "Interrupted creation, marking failed");
                    let _ = machinery::update_machine_status(
                        &self.db,
                        machine_id,
                        MachineStatusDb::Failed,
                        Some("Interrupted by restart"),
                    )
                    .await;
                }

                MachineStatusDb::Provisioning => {
                    warn!(machine_id, "Interrupted provisioning, restoring to ready");
                    let _ = machinery::update_machine_status(
                        &self.db,
                        machine_id,
                        MachineStatusDb::Ready,
                        None,
                    )
                    .await;
                }

                MachineStatusDb::Assigned | MachineStatusDb::Reverting => {
                    warn!(machine_id, status = ?db_machine.status, "Machine was in-flight, attempting recovery");
                    let _ = machinery::release_machine(&self.db, machine_id).await;

                    if self
                        .try_snapshot_revert(machine_id, db_machine, &provider_vms)
                        .await
                    {
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

                MachineStatusDb::Ready => {
                    self.reconcile_ready_machine(machine_id, db_machine, &provider_vms)
                        .await;
                }

                MachineStatusDb::Deleting => {
                    warn!(machine_id, "Machine was mid-deletion, finishing cleanup");
                    if let Some(ref provider_id) = db_machine.provider_id {
                        let _ = destroy_by_provider_id(&self.provider, provider_id).await;
                    }
                    let _ = machinery::delete_machine(&self.db, machine_id).await;
                }

                MachineStatusDb::Failed => {
                    debug!(machine_id, "Machine already failed, skipping");
                }
            }
        }

        // Clean up stale provider snapshots not tracked in the DB.
        if let Some(snapshot_cap) = self.provider.snapshot() {
            for db_machine in &db_machines {
                let machine_id = match db_machine.id {
                    Some(id) => id,
                    None => continue,
                };
                let provider_id = match db_machine.provider_id.as_deref() {
                    Some(id) => id,
                    None => continue,
                };
                let runtime_vm = match provider_vms.iter().find(|vm| vm.id.0 == provider_id) {
                    Some(vm) => vm,
                    None => continue,
                };

                let provider_snaps = match snapshot_cap.list_snapshots(runtime_vm).await {
                    Ok(snaps) => snaps,
                    Err(_) => continue,
                };
                let db_snaps = snapshots::fetch_snapshots_for_machine(&self.db, machine_id)
                    .await
                    .unwrap_or_default();

                for ps in &provider_snaps {
                    if !db_snaps.iter().any(|ds| ds.provider_snapshot_id == ps.id.0) {
                        warn!(
                            machine_id,
                            snapshot = ps.name.as_str(),
                            provider_id = %ps.id,
                            "Deleting stale provider snapshot not tracked in DB"
                        );
                        if let Err(e) = snapshot_cap.delete_snapshot(&ps.id).await {
                            warn!(
                                machine_id,
                                snapshot = ps.name.as_str(),
                                error = %e,
                                "Failed to delete stale provider snapshot"
                            );
                        }
                    }
                }
            }
        }

        info!("Reconciliation complete");
        Ok(())
    }

    async fn reconcile_ready_machine(
        &self,
        machine_id: i32,
        db_machine: &DbMachine,
        provider_vms: &[malbox_machinery::Machine],
    ) {
        let provider_id = match &db_machine.provider_id {
            Some(id) => id,
            None => {
                warn!(
                    machine_id,
                    "Ready machine missing provider_id, marking failed"
                );
                let _ = machinery::update_machine_status(
                    &self.db,
                    machine_id,
                    MachineStatusDb::Failed,
                    Some("Missing provider_id"),
                )
                .await;
                return;
            }
        };

        let vm_exists = provider_vms.iter().any(|vm| vm.id.0 == *provider_id);
        if !vm_exists {
            warn!(machine_id, %provider_id, "Ready machine not found in provider, marking failed");
            let _ = machinery::update_machine_status(
                &self.db,
                machine_id,
                MachineStatusDb::Failed,
                Some("VM not found in provider after restart"),
            )
            .await;
            return;
        }

        // Verify the active snapshot still exists in the provider
        if let Some(snapshot_cap) = self.provider.snapshot() {
            let active_snap = match snapshots::fetch_active_snapshot(&self.db, machine_id).await {
                Ok(Some(snap)) => snap,
                Ok(None) => return,
                Err(_) => return,
            };

            let runtime_vm = match provider_vms.iter().find(|vm| vm.id.0 == *provider_id) {
                Some(vm) => vm,
                None => return,
            };

            match snapshot_cap.list_snapshots(runtime_vm).await {
                Ok(snaps) => {
                    if !snaps
                        .iter()
                        .any(|s| s.id.0 == active_snap.provider_snapshot_id)
                    {
                        warn!(
                            machine_id,
                            snapshot = active_snap.name,
                            "Active snapshot not found in provider, marking failed"
                        );
                        let _ = machinery::update_machine_status(
                            &self.db,
                            machine_id,
                            MachineStatusDb::Failed,
                            Some("Active snapshot not found in provider"),
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

    /// Attempt a snapshot-based revert during reconciliation.
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

        let active_snap = match snapshots::fetch_active_snapshot(&self.db, machine_id).await {
            Ok(Some(snap)) => snap,
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

        if snapshot_cap
            .restore_snapshot(runtime_vm, &SnapshotId(active_snap.provider_snapshot_id))
            .await
            .is_err()
        {
            return false;
        }

        if machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Ready, None)
            .await
            .is_ok()
        {
            self.machine_available.notify_waiters();
            true
        } else {
            false
        }
    }

    /// Reconcile DB machines against config-declared machines.
    pub async fn reconcile_config(&self, declared: &[MachineConfig]) -> Result<()> {
        if declared.is_empty() {
            warn!("Config declares zero machines — all existing machines will be destroyed");
        }

        let db_machines = machinery::fetch_all_machines(&self.db)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        let declared_by_name: std::collections::HashMap<&str, &MachineConfig> =
            declared.iter().map(|m| (m.name.as_str(), m)).collect();

        let db_by_name: std::collections::HashMap<&str, &DbMachine> =
            db_machines.iter().map(|m| (m.name.as_str(), m)).collect();

        // Destroy machines not in config (skip assigned)
        for db_machine in &db_machines {
            if declared_by_name.contains_key(db_machine.name.as_str()) {
                continue;
            }
            let machine_id = match db_machine.id {
                Some(id) => id,
                None => continue,
            };
            if db_machine.status == MachineStatusDb::Assigned {
                warn!(machine_id, name = %db_machine.name, "Machine not in config but assigned, skipping");
                continue;
            }
            info!(machine_id, name = %db_machine.name, "Machine not in config, destroying");
            if let Some(ref provider_id) = db_machine.provider_id {
                let _ = destroy_by_provider_id(&self.provider, provider_id).await;
            }
            let _ = machinery::delete_machine(&self.db, machine_id).await;
        }

        // Create or reconcile machines in config
        for config in declared {
            match db_by_name.get(config.name.as_str()) {
                None => {
                    info!(name = %config.name, "Creating new machine from config");
                    self.create_from_config(config).await;
                }
                Some(db_machine) => {
                    let machine_id = match db_machine.id {
                        Some(id) => id,
                        None => continue,
                    };
                    if self.specs_match(config, db_machine) {
                        if db_machine.status == MachineStatusDb::Failed {
                            info!(machine_id, name = %config.name, "Retrying failed machine");
                            self.retry_failed_machine(machine_id, db_machine, config)
                                .await;
                        }
                        // else: no-op, specs match and machine is healthy
                    } else {
                        if db_machine.status == MachineStatusDb::Assigned {
                            warn!(machine_id, name = %config.name, "Specs changed but machine is assigned, skipping");
                            continue;
                        }
                        info!(machine_id, name = %config.name, "Specs changed, destroying and recreating");
                        if let Some(ref provider_id) = db_machine.provider_id {
                            let _ = destroy_by_provider_id(&self.provider, provider_id).await;
                        }
                        let _ = machinery::delete_machine(&self.db, machine_id).await;
                        self.create_from_config(config).await;
                    }
                }
            }
        }

        info!("Config reconciliation complete");
        Ok(())
    }

    fn specs_match(&self, config: &MachineConfig, db: &DbMachine) -> bool {
        let config_platform: machinery::MachinePlatform = config.platform.into();
        let config_arch: MachineArch = config.arch.into();
        let config_hash = hash_provider_config(&config.provider_config);

        db.platform == config_platform
            && db.arch == config_arch
            && db.image_name.as_deref() == Some(&config.image)
            && db.cpus == Some(config.cpus as i32)
            && db.memory_mb == Some(config.memory as i32)
            && db.disk_size_mb == Some(config.disk_size as i64)
            && db.provider_config_hash.as_deref() == Some(&config_hash)
    }

    async fn create_from_config(&self, config: &MachineConfig) {
        let image = match images::fetch_image_by_name(&self.db, &config.image).await {
            Ok(Some(img)) => img,
            Ok(None) => {
                error!(name = %config.name, image = %config.image, "Image not found, skipping machine");
                return;
            }
            Err(e) => {
                error!(name = %config.name, error = %e, "Failed to fetch image, skipping machine");
                return;
            }
        };

        let config_hash = hash_provider_config(&config.provider_config);

        let db_machine = match machinery::insert_machine(
            &self.db,
            &config.name,
            config.platform.into(),
            config.arch.into(),
            image.id,
            &config.image,
            config.cpus as i32,
            config.memory as i32,
            config.disk_size as i64,
            Some(&config_hash),
        )
        .await
        {
            Ok(m) => m,
            Err(e) => {
                error!(name = %config.name, error = %e, "Failed to insert machine");
                return;
            }
        };

        let machine_id = db_machine.id.expect("insert_machine returns id");

        if let Err(e) = self
            .setup_machine_from_config(machine_id, &image.path, config)
            .await
        {
            error!(machine_id, error = %e, "Machine setup failed");
            let _ = machinery::update_machine_status(
                &self.db,
                machine_id,
                MachineStatusDb::Failed,
                Some(&e.to_string()),
            )
            .await;
            return;
        }

        // Mark Ready before provisioning steps (run_provision_step requires Ready or Failed)
        if let Err(e) =
            machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Ready, None)
                .await
        {
            error!(machine_id, error = %e, "Failed to mark machine ready");
            return;
        }

        self.machine_available.notify_waiters();
        info!(machine_id, "Machine is ready");
    }

    /// Retry a failed machine by reusing its existing DB row.
    /// Destroys any leftover provider VM, cleans up snapshots,
    /// resets the row to creating, and re-runs setup.
    async fn retry_failed_machine(
        &self,
        machine_id: i32,
        db_machine: &DbMachine,
        config: &MachineConfig,
    ) {
        // Destroy leftover provider VM if any
        if let Some(ref provider_id) = db_machine.provider_id {
            let _ = destroy_by_provider_id(&self.provider, provider_id).await;
        }

        // Clean up old snapshots
        let _ = snapshots::delete_snapshots_for_machine(&self.db, machine_id).await;

        // Resolve image
        let image = match images::fetch_image_by_name(&self.db, &config.image).await {
            Ok(Some(img)) => img,
            Ok(None) => {
                error!(machine_id, name = %config.name, image = %config.image, "Image not found, skipping retry");
                return;
            }
            Err(e) => {
                error!(machine_id, name = %config.name, error = %e, "Failed to fetch image, skipping retry");
                return;
            }
        };

        // Reset to creating (reuse existing row)
        if let Err(e) =
            machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Creating, None)
                .await
        {
            error!(machine_id, error = %e, "Failed to reset machine status");
            return;
        }

        // Re-run setup using the existing machine ID
        if let Err(e) = self
            .setup_machine_from_config(machine_id, &image.path, config)
            .await
        {
            error!(machine_id, error = %e, "Machine retry setup failed");
            let _ = machinery::update_machine_status(
                &self.db,
                machine_id,
                MachineStatusDb::Failed,
                Some(&e.to_string()),
            )
            .await;
            return;
        }

        if let Err(e) =
            machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Ready, None)
                .await
        {
            error!(machine_id, error = %e, "Failed to mark machine ready after retry");
            return;
        }

        self.machine_available.notify_waiters();
        info!(machine_id, "Machine retry successful, now ready");
    }
}
