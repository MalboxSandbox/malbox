//! Machine revert logic — restore to clean state after task completion.

use crate::error::{ResourceError, Result};
use malbox_database::repositories::images;
use malbox_database::repositories::machinery::{self, MachineStatusDb};
use malbox_database::repositories::snapshots;
use malbox_machinery::ProviderHandle;
use malbox_machinery::provider::capabilities::snapshot::SnapshotId;
use tracing::info;

use super::MachinePool;

impl MachinePool {
    /// Revert a machine to its active snapshot after task completion.
    ///
    /// If the provider supports snapshots and the machine has an active
    /// snapshot in the DB, restores it. Otherwise, deallocates and
    /// re-provisions from the original image.
    pub(crate) async fn revert_machine(&self, machine_id: i32) -> Result<()> {
        let db_machine = machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(ResourceError::Database)?
            .ok_or(ResourceError::MachineNotFound {
                id: machine_id.to_string(),
            })?;

        // Try snapshot-based revert first
        if let Some(provider_id) = db_machine.provider_id.as_deref() {
            let active_snap = snapshots::fetch_active_snapshot(&self.db, machine_id)
                .await
                .map_err(ResourceError::Database)?;

            if let Some(snap) = active_snap
                && self
                    .restore_provider_snapshot(machine_id, provider_id, &snap.provider_snapshot_id)
                    .await?
            {
                machinery::update_machine_status(
                    &self.db,
                    machine_id,
                    MachineStatusDb::Ready,
                    None,
                )
                .await
                .map_err(ResourceError::Database)?;

                self.machine_available.notify_waiters();
                info!(
                    machine_id,
                    snapshot = snap.name,
                    "Machine reverted via snapshot"
                );
                return Ok(());
            }
        }

        // Fallback: deallocate and re-provision from image
        info!(
            machine_id,
            "No active snapshot available, re-provisioning from image"
        );

        if let Some(ref provider_id) = db_machine.provider_id {
            let _ = destroy_by_provider_id(&self.provider, provider_id).await;
        }

        // Clean up old snapshot records before re-provisioning
        let _ = snapshots::delete_snapshots_for_machine(&self.db, machine_id).await;

        let image_id = db_machine
            .image_id
            .ok_or_else(|| ResourceError::Internal("Machine has no image_id".to_string()))?;
        let image_path = images::fetch_image_by_id(&self.db, image_id)
            .await
            .map_err(ResourceError::Database)?
            .ok_or_else(|| ResourceError::Internal(format!("Image with id {image_id} not found")))?
            .path;

        machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Creating, None)
            .await
            .map_err(ResourceError::Database)?;

        self.setup_machine(machine_id, &image_path).await
    }

    /// Restore a provider-side snapshot onto the machine's VM.
    ///
    /// Returns `false` when the provider has no snapshot capability or the VM
    /// is not known to the provider; nothing is restored in that case.
    pub(crate) async fn restore_provider_snapshot(
        &self,
        machine_id: i32,
        provider_id: &str,
        provider_snapshot_id: &str,
    ) -> Result<bool> {
        let Some(snapshot_cap) = self.provider.snapshot() else {
            return Ok(false);
        };
        let provider_vms = self
            .provider
            .allocate()
            .list()
            .await
            .map_err(|e| ResourceError::Provider(e.to_string()))?;
        let Some(runtime_vm) = provider_vms.iter().find(|vm| vm.id.0 == provider_id) else {
            return Ok(false);
        };
        snapshot_cap
            .restore_snapshot(runtime_vm, &SnapshotId(provider_snapshot_id.to_string()))
            .await
            .map_err(|e| ResourceError::SnapshotRestoreFailed {
                id: machine_id,
                reason: e.to_string(),
            })?;
        Ok(true)
    }
}

/// Destroy a VM by its provider-side ID.
pub(crate) async fn destroy_by_provider_id(
    provider: &ProviderHandle,
    provider_id: &str,
) -> Result<()> {
    let allocate = provider.allocate();
    let provider_vms = allocate
        .list()
        .await
        .map_err(|e| ResourceError::Provider(e.to_string()))?;

    if let Some(vm) = provider_vms.iter().find(|vm| vm.id.0 == provider_id) {
        allocate
            .destroy(vm)
            .await
            .map_err(|e| ResourceError::DeallocationFailed(e.to_string()))?;
    }

    Ok(())
}
