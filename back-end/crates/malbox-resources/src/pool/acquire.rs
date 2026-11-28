//! Request-driven machine acquisition, snapshot preparation and failure
//! marking for detonations.

use super::{DbMachine, MachinePool};
use crate::error::{ResourceError, Result};
use malbox_database::repositories::machinery::{
    self, AcquireRow, MachineArch, MachineFilter, MachinePlatform, MachineStatusDb,
};
use malbox_database::repositories::snapshots::{self, MachineSnapshot};
use tracing::{info, warn};
use uuid::Uuid;

/// What a detonation needs from a machine and its snapshot.
#[derive(Debug, Clone, Default)]
pub struct MachineRequest {
    /// Required guest platform, or any.
    pub platform: Option<MachinePlatform>,
    /// Required CPU architecture, or any.
    pub arch: Option<MachineArch>,
    /// Pin the request to this machine (database ID).
    pub machine_id: Option<i32>,
    /// Run on this exact snapshot.
    pub snapshot_id: Option<Uuid>,
    /// Run on the snapshot with this name.
    pub snapshot_name: Option<String>,
    /// Tags the machine must carry.
    pub tags: Vec<String>,
    /// Guest plugins the snapshot must have installed.
    pub required_guest_plugins: Vec<String>,
}

/// Result of trying to acquire a machine for a task.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "short-lived return value; boxing buys nothing"
)]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox-scheduler to map acquisition results"
)]
pub enum AcquireOutcome {
    /// The machine is assigned to the task; `snapshot` is the one to run on.
    Acquired {
        /// The leased machine row.
        machine: DbMachine,
        /// Snapshot the task should run on.
        snapshot: MachineSnapshot,
    },
    /// A matching machine exists but none is ready.
    Busy,
    /// No machine can serve the request.
    Unserviceable(String),
}

impl MachineRequest {
    fn to_filter(&self) -> MachineFilter {
        MachineFilter {
            platform: self.platform.clone(),
            arch: self.arch.clone(),
            machine_id: self.machine_id,
            snapshot_id: self.snapshot_id,
            snapshot_name: self.snapshot_name.clone(),
            tags: self.tags.clone(),
            required_guest_plugins: self.required_guest_plugins.clone(),
        }
    }

    fn unserviceable_message(&self) -> String {
        format!(
            "no machine matches platform={:?} arch={:?} machine_id={:?} snapshot={:?} tags={:?} guest_plugins={:?}",
            self.platform,
            self.arch,
            self.machine_id,
            self.snapshot_id
                .map(|id| id.to_string())
                .or_else(|| self.snapshot_name.clone()),
            self.tags,
            self.required_guest_plugins,
        )
    }
}

impl MachinePool {
    /// Assign a ready machine matching `req` to `task_id`.
    ///
    /// # Errors
    ///
    /// Fails if a database query fails while matching or loading the machine.
    pub async fn acquire_for(&self, req: &MachineRequest, task_id: i32) -> Result<AcquireOutcome> {
        let row = machinery::acquire_machine_for(&self.db, &req.to_filter(), task_id)
            .await
            .map_err(ResourceError::Database)?;

        match row {
            AcquireRow::Busy => Ok(AcquireOutcome::Busy),
            AcquireRow::Unserviceable => {
                Ok(AcquireOutcome::Unserviceable(req.unserviceable_message()))
            }
            AcquireRow::Acquired {
                machine_id,
                snapshot_id,
            } => {
                let loaded = self.load_acquired(machine_id, snapshot_id).await;
                if loaded.is_err() {
                    // The machine is already assigned; give it back or it
                    // stays reserved until the next startup reconcile.
                    if let Err(e) = self.release(machine_id).await {
                        warn!(machine_id, error = %e, "failed to release machine after acquire error");
                    }
                }
                let (machine, snapshot) = loaded?;
                Ok(AcquireOutcome::Acquired { machine, snapshot })
            }
        }
    }

    async fn load_acquired(
        &self,
        machine_id: i32,
        snapshot_id: Uuid,
    ) -> Result<(DbMachine, MachineSnapshot)> {
        let machine = machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(ResourceError::Database)?
            .ok_or(ResourceError::MachineNotFound {
                id: machine_id.to_string(),
            })?;
        if machine.id.is_none() {
            return Err(ResourceError::MachineNotFound {
                id: machine_id.to_string(),
            });
        }
        let snapshot = snapshots::fetch_snapshot_by_id(&self.db, snapshot_id)
            .await
            .map_err(ResourceError::Database)?
            .ok_or(ResourceError::SnapshotNotFound {
                id: snapshot_id.to_string(),
            })?;
        Ok((machine, snapshot))
    }

    /// Make `snapshot` the machine's running state.
    ///
    /// The active snapshot is what the machine was reverted to on release,
    /// so it needs no work. Any other snapshot is activated (so the next
    /// revert restores it too) and restored on the provider.
    ///
    /// # Errors
    ///
    /// Fails if activation fails in the database, the provider lacks snapshot
    /// support, or the machine cannot be restored at the provider.
    pub async fn prepare_snapshot(
        &self,
        machine: &DbMachine,
        snapshot: &MachineSnapshot,
    ) -> Result<()> {
        if snapshot.is_active {
            return Ok(());
        }
        let machine_id = snapshot.machine_id;
        snapshots::activate_snapshot(&self.db, snapshot.id)
            .await
            .map_err(ResourceError::Database)?;

        let fail = |reason: &str| ResourceError::SnapshotRestoreFailed {
            id: machine_id,
            reason: reason.to_string(),
        };
        if self.provider.snapshot().is_none() {
            return Err(fail("provider has no snapshot capability"));
        }
        let provider_id = machine
            .provider_id
            .as_deref()
            .ok_or_else(|| fail("machine has no provider id"))?;
        if !self
            .restore_provider_snapshot(machine_id, provider_id, &snapshot.provider_snapshot_id)
            .await?
        {
            return Err(fail("machine not found at provider"));
        }
        info!(machine_id, snapshot = %snapshot.name, "Restored non-active snapshot for detonation");
        Ok(())
    }

    /// Take a machine out of rotation after an unrecoverable session error.
    ///
    /// # Errors
    ///
    /// Fails if the machine cannot be released or marked failed in the database.
    pub async fn mark_failed(&self, machine_id: i32, reason: &str) -> Result<()> {
        machinery::release_machine(&self.db, machine_id)
            .await
            .map_err(ResourceError::Database)?;
        machinery::update_machine_status(
            &self.db,
            machine_id,
            MachineStatusDb::Failed,
            Some(reason),
        )
        .await
        .map_err(ResourceError::Database)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unserviceable_message_lists_every_constraint() {
        let req = MachineRequest {
            platform: Some(MachinePlatform::Windows),
            arch: Some(MachineArch::X64),
            machine_id: None,
            snapshot_id: None,
            snapshot_name: Some("with-plugins".into()),
            tags: vec!["gpu".into()],
            required_guest_plugins: vec!["mon".into()],
        };
        assert_eq!(
            req.unserviceable_message(),
            "no machine matches platform=Some(Windows) arch=Some(X64) machine_id=None \
             snapshot=Some(\"with-plugins\") tags=[\"gpu\"] guest_plugins=[\"mon\"]"
        );
    }

    #[test]
    fn unserviceable_message_prefers_snapshot_id() {
        let id = Uuid::nil();
        let req = MachineRequest {
            snapshot_id: Some(id),
            snapshot_name: Some("ignored".into()),
            ..Default::default()
        };
        assert!(
            req.unserviceable_message()
                .contains(&format!("snapshot=Some(\"{id}\")"))
        );
    }
}
