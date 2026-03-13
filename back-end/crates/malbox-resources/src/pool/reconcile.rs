//! Startup reconciliation — validate DB state against the provider.

use crate::error::{ResourceError, Result};
use malbox_database::repositories::machinery::{self, MachineStatusDb};
use tracing::{info, warn};

use super::revert::deallocate_by_provider_id;
use super::{DbMachine, MachinePool};

impl MachinePool {
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
                        let _ = deallocate_by_provider_id(&self.provider, provider_id).await;
                    }
                    let _ = machinery::delete_machine(&self.db, machine_id).await;
                }

                MachineStatusDb::Failed => {
                    info!(machine_id, "Machine already failed, skipping");
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

        if let Some(snapshot_cap) = self.provider.snapshot() {
            if let Some(ref snap_name) = db_machine.clean_snapshot {
                let runtime_vm = provider_vms.iter().find(|vm| vm.id.0 == *provider_id);
                if let Some(vm) = runtime_vm {
                    match snapshot_cap.list_snapshots(vm).await {
                        Ok(snaps) => {
                            if !snaps.iter().any(|s| s.name == *snap_name) {
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
}
