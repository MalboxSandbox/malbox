//! Machine pool for managing persistent, user-created machines.

mod acquire;
mod provisioning;
mod reconcile;
mod revert;

pub use acquire::{AcquireOutcome, MachineRequest};

use crate::error::{ResourceError, Result};
use malbox_database::PgPool;
use malbox_database::repositories::machinery::{self, MachinePlatform, MachineStatusDb};
use malbox_machinery::ProviderHandle;
use malbox_machinery::machine::MachineEndpoint;
use malbox_machinery::{Machine as RuntimeMachine, MachineId, Platform};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use tokio::sync::Notify;
use tracing::{debug, error, info};

/// Database row describing a pool machine.
pub type DbMachine = machinery::Machine;

/// Central machine pool that owns all lifecycle operations.
///
/// All heavy work (provisioning, reverting) is spawned into background tokio
/// tasks so that API calls return immediately.  The `machine_available`
/// [`Notify`] is signalled whenever a machine transitions to `ready`.
#[derive(Clone, Debug)]
pub struct MachinePool {
    pub(crate) provider: Arc<ProviderHandle>,
    pub(crate) db: PgPool,
    /// Signalled whenever a machine becomes `ready`; acquirers wait on it.
    pub machine_available: Arc<Notify>,
    /// Config-declared `provider_config` per machine name. The DB stores only
    /// its hash, so revert-by-reprovision reads the value from here.
    /// Populated by `reconcile_config`.
    pub(crate) provider_configs: Arc<RwLock<HashMap<String, toml::Value>>>,
}

impl MachinePool {
    /// Create a pool backed by `provider` and the machine tables in `db`.
    pub fn new(provider: Arc<ProviderHandle>, db: PgPool) -> Self {
        Self {
            provider,
            db,
            machine_available: Arc::new(Notify::new()),
            provider_configs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Release a machine after task completion.
    ///
    /// Marks the machine as `reverting` and spawns an async revert.
    ///
    /// # Errors
    ///
    /// Fails if the machine cannot be marked as released in the database.
    pub async fn release(&self, machine_id: i32) -> Result<()> {
        machinery::release_machine(&self.db, machine_id)
            .await
            .map_err(ResourceError::Database)?;

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

    /// Fetch a single machine by ID.
    ///
    /// # Errors
    ///
    /// Fails if the database query fails.
    pub async fn get(&self, machine_id: i32) -> Result<Option<DbMachine>> {
        machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(ResourceError::Database)
    }

    /// List all machines.
    ///
    /// # Errors
    ///
    /// Fails if the database query fails.
    pub async fn list(&self) -> Result<Vec<DbMachine>> {
        machinery::fetch_all_machines(&self.db)
            .await
            .map_err(ResourceError::Database)
    }

    /// Delete a snapshot from both the provider and the database.
    ///
    /// Provider deletion is best-effort: if it fails the DB record is still
    /// removed so the system doesn't get stuck with an orphaned reference.
    ///
    /// # Errors
    ///
    /// Fails if deleting the snapshot's provision runs or its row fails.
    pub async fn delete_snapshot(
        &self,
        machine_id: i32,
        snapshot: &malbox_database::repositories::snapshots::MachineSnapshot,
    ) -> Result<()> {
        use malbox_database::repositories::{provision_runs, snapshots};
        use malbox_machinery::provider::capabilities::snapshot::SnapshotId;

        if let Some(snapshot_cap) = self.provider.snapshot() {
            let snap_id = SnapshotId(snapshot.provider_snapshot_id.clone());
            if let Err(e) = snapshot_cap.delete_snapshot(&snap_id).await {
                tracing::warn!(
                    machine_id,
                    snapshot = snapshot.name.as_str(),
                    error = %e,
                    "Failed to delete snapshot from provider (continuing with DB cleanup)"
                );
            }
        }

        provision_runs::delete_provision_runs_for_snapshot(&self.db, snapshot.id)
            .await
            .map_err(ResourceError::Database)?;

        snapshots::delete_snapshot(&self.db, snapshot.id)
            .await
            .map_err(ResourceError::Database)?;

        info!(
            machine_id,
            snapshot = snapshot.name.as_str(),
            "Snapshot deleted"
        );
        Ok(())
    }

    /// Fetch the guest plugin names from the active snapshot for a machine.
    ///
    /// Returns an empty list if there is no active snapshot or the snapshot
    /// has no guest plugins recorded.
    ///
    /// # Errors
    ///
    /// Fails if the active snapshot cannot be fetched from the database.
    pub async fn get_active_snapshot_guest_plugins(&self, machine_id: i32) -> Result<Vec<String>> {
        use malbox_database::repositories::snapshots;

        let snap = snapshots::fetch_active_snapshot(&self.db, machine_id)
            .await
            .map_err(ResourceError::Database)?;

        let plugins = snap
            .and_then(|s| s.guest_plugins)
            .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
            .unwrap_or_default();

        Ok(plugins)
    }

    /// Start an acquired machine's VM and wait for the guest plugin gRPC port
    /// (50051) to become reachable.
    ///
    /// After a snapshot revert the VM is stopped. Workers must call this before
    /// attempting to connect to guest plugins.
    ///
    /// # Errors
    ///
    /// Fails if the machine lacks a provider ID or valid IP, the provider
    /// cannot start it, or the port is unreachable within two minutes.
    pub async fn start_and_wait(&self, db_machine: &DbMachine) -> Result<()> {
        let provider_id = db_machine
            .provider_id
            .as_deref()
            .ok_or_else(|| ResourceError::Internal("Machine has no provider_id".into()))?;

        let ip: IpAddr = db_machine
            .ip
            .as_deref()
            .ok_or_else(|| ResourceError::Internal("Machine has no IP".into()))?
            .parse()
            .map_err(|e| ResourceError::Internal(format!("Invalid IP: {e}")))?;

        let platform = match db_machine.platform {
            MachinePlatform::Windows => Platform::Windows,
            MachinePlatform::Linux => Platform::Linux,
        };

        // Build a runtime machine so the provider can look up the domain.
        let mut runtime_machine = RuntimeMachine::new(MachineId(provider_id.to_string()));
        runtime_machine.set_endpoint(Some(MachineEndpoint {
            address: ip,
            id: provider_id.to_string(),
            platform,
        }));

        // Start the VM (idempotent if already running).
        let allocate = self.provider.allocate();
        allocate
            .start(&runtime_machine)
            .await
            .map_err(|e| ResourceError::Provider(format!("Failed to start VM: {e}")))?;

        info!(
            machine_id = db_machine.id,
            %ip,
            "VM started, waiting for guest plugin port"
        );

        // Wait for the guest plugin gRPC port (50051) to become reachable.
        let addr = std::net::SocketAddr::new(ip, 50051);
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_mins(2);
        let connect_timeout = std::time::Duration::from_secs(5);
        let mut interval = std::time::Duration::from_millis(200);
        let max_interval = std::time::Duration::from_secs(10);

        loop {
            if let Ok(Ok(_)) =
                tokio::time::timeout(connect_timeout, tokio::net::TcpStream::connect(addr)).await
            {
                info!(
                    %addr,
                    elapsed_secs = start.elapsed().as_secs(),
                    "Guest plugin port reachable"
                );
                return Ok(());
            }

            if start.elapsed() >= timeout {
                return Err(ResourceError::Timeout(format!(
                    "Guest plugin at {} not reachable within {}s",
                    addr,
                    timeout.as_secs(),
                )));
            }

            debug!(
                %addr,
                elapsed_secs = start.elapsed().as_secs(),
                "Waiting for guest plugin..."
            );
            tokio::time::sleep(interval).await;
            interval = (interval * 2).min(max_interval);
        }
    }
}
