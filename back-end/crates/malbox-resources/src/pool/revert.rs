//! Machine revert logic — restore to clean state after task completion.

use crate::error::{ResourceError, Result};
use malbox_database::repositories::machinery::{self, MachineStatusDb};
use malbox_database::repositories::images;
use malbox_machinery::ProviderHandle;
use tracing::info;

use super::MachinePool;

impl MachinePool {
    /// Revert a machine to its clean state after task completion.
    ///
    /// If the provider supports snapshots and the machine has a recorded
    /// snapshot, restores it.  Otherwise, deallocates and re-provisions
    /// from the original image.
    pub(crate) async fn revert_machine(&self, machine_id: i32) -> Result<()> {
        let db_machine = machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or(ResourceError::MachineNotFound { id: machine_id.to_string() })?;

        // Try snapshot-based revert first
        if let (Some(snapshot_cap), Some(snap_name), Some(provider_id)) = (
            self.provider.snapshot(),
            db_machine.clean_snapshot.as_deref(),
            db_machine.provider_id.as_deref(),
        ) {
            let allocate = self.provider.allocate();
            let provider_vms = allocate
                .list()
                .await
                .map_err(|e| ResourceError::Provider(e.to_string()))?;

            if let Some(runtime_vm) = provider_vms.iter().find(|vm| vm.id.0 == provider_id) {
                let snapshots = snapshot_cap
                    .list_snapshots(runtime_vm)
                    .await
                    .map_err(|e| ResourceError::Provider(e.to_string()))?;

                if let Some(snap_info) = snapshots.iter().find(|s| s.name == *snap_name) {
                    snapshot_cap
                        .restore_snapshot(runtime_vm, &snap_info.id)
                        .await
                        .map_err(|e| ResourceError::SnapshotRestoreFailed {
                            id: machine_id,
                            reason: e.to_string(),
                        })?;

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

        if let Some(ref provider_id) = db_machine.provider_id {
            let _ = deallocate_by_provider_id(&self.provider, provider_id).await;
        }

        let image_id = db_machine.image_id
            .ok_or_else(|| ResourceError::Internal("Machine has no image_id".to_string()))?;
        let image_path = images::fetch_image_by_id(&self.db, image_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or_else(|| ResourceError::Internal(format!("Image with id {} not found", image_id)))?
            .path;

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
}

/// Deallocate a VM by its provider-side ID.
pub(crate) async fn deallocate_by_provider_id(
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
            .deallocate(vm)
            .await
            .map_err(|e| ResourceError::DeallocationFailed(e.to_string()))?;
    }

    Ok(())
}
