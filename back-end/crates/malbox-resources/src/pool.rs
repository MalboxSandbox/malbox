//! Machine pool for managing persistent, user-created machines.

mod provisioning;
mod reconcile;
mod revert;

use crate::error::{ResourceError, Result};
use malbox_config::machinery::MachineDefaults;
use malbox_database::PgPool;
use malbox_database::repositories::images;
use malbox_database::repositories::machinery::{
    self, MachineArch, MachinePlatform, MachineStatusDb,
};
use malbox_machinery::ProviderHandle;
use malbox_machinery::provisioner::Provisioner;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{error, info, warn};

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
#[derive(Clone)]
pub struct MachinePool {
    pub(crate) provider: Arc<ProviderHandle>,
    pub(crate) provisioner: Option<Arc<dyn Provisioner>>,
    pub(crate) db: PgPool,
    pub(crate) config: MachinePoolConfig,
    pub machine_available: Arc<Notify>,
}

impl MachinePool {
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

    /// Create a new machine.
    ///
    /// Validates that the requested image exists, inserts a DB row in
    /// `creating` status, and spawns background provisioning. Returns the
    /// DB row immediately so the caller can track progress.
    pub async fn create_machine(&self, request: CreateMachineRequest) -> Result<DbMachine> {
        let image = images::fetch_image_by_name(&self.db, &request.image)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or_else(|| {
                ResourceError::Internal(format!("Image '{}' not found", request.image,))
            })?;

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

        let pool = self.clone();
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
        machinery::release_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        let pool = self.clone();
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
        let db_machine = machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or(ResourceError::MachineNotFound { id: machine_id.to_string() })?;

        if db_machine.status == MachineStatusDb::Assigned {
            return Err(ResourceError::MachineAssigned { id: machine_id });
        }

        machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Deleting, None)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        if let Some(ref provider_id) = db_machine.provider_id {
            if let Err(e) = revert::deallocate_by_provider_id(&self.provider, provider_id).await {
                warn!(machine_id, error = %e, "Failed to deallocate from provider during delete (continuing)");
            }
        }

        machinery::delete_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        info!(machine_id, "Machine deleted");
        Ok(())
    }

    /// Retry provisioning of a failed machine.
    pub async fn retry(&self, machine_id: i32) -> Result<DbMachine> {
        let db_machine = machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or(ResourceError::MachineNotFound { id: machine_id.to_string() })?;

        if db_machine.status != MachineStatusDb::Failed {
            return Err(ResourceError::InvalidMachineState {
                id: machine_id,
                status: format!("{:?}", db_machine.status),
                expected: "failed".to_string(),
            });
        }

        let updated =
            machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Creating, None)
                .await
                .map_err(|e| ResourceError::Database(e.to_string()))?;

        let image_id = db_machine.image_id
            .ok_or_else(|| ResourceError::Internal("Machine has no image_id".to_string()))?;
        let image_path = images::fetch_image_by_id(&self.db, image_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or_else(|| ResourceError::Internal(format!("Image with id {} not found", image_id)))?
            .path;

        let pool = self.clone();
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
}
