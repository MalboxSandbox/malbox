//! Machine pool for managing persistent, user-created machines.

mod provisioning;
mod reconcile;
mod revert;

use crate::error::{ResourceError, Result};
use malbox_database::PgPool;
use malbox_database::repositories::machinery::{self, MachinePlatform, MachineStatusDb};
use malbox_machinery::ProviderHandle;
use malbox_machinery::machine::MachineEndpoint;
use malbox_machinery::{Machine as RuntimeMachine, MachineId, Platform};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{error, info, warn};

pub type DbMachine = machinery::Machine;

/// Central machine pool that owns all lifecycle operations.
///
/// All heavy work (provisioning, reverting) is spawned into background tokio
/// tasks so that API calls return immediately.  The `machine_available`
/// [`Notify`] is signalled whenever a machine transitions to `ready`.
#[derive(Clone)]
pub struct MachinePool {
    pub(crate) provider: Arc<ProviderHandle>,
    pub(crate) db: PgPool,
    pub machine_available: Arc<Notify>,
}

impl MachinePool {
    pub fn new(provider: Arc<ProviderHandle>, db: PgPool) -> Self {
        Self {
            provider,
            db,
            machine_available: Arc::new(Notify::new()),
        }
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

    /// Fetch the guest plugin names from the active snapshot for a machine.
    ///
    /// Returns an empty list if there is no active snapshot or the snapshot
    /// has no guest plugins recorded.
    pub async fn get_active_snapshot_guest_plugins(&self, machine_id: i32) -> Result<Vec<String>> {
        use malbox_database::repositories::snapshots;

        let snap = snapshots::fetch_active_snapshot(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

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
            .map_err(|e| ResourceError::Internal(format!("Invalid IP: {}", e)))?;

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
            .map_err(|e| ResourceError::Provider(format!("Failed to start VM: {}", e)))?;

        info!(
            machine_id = db_machine.id,
            %ip,
            "VM started, waiting for guest plugin port"
        );

        // Wait for the guest plugin gRPC port (50051) to become reachable.
        let addr = std::net::SocketAddr::new(ip, 50051);
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(120);
        let connect_timeout = std::time::Duration::from_secs(5);
        let mut interval = std::time::Duration::from_secs(3);
        let max_interval = std::time::Duration::from_secs(10);

        loop {
            match tokio::time::timeout(connect_timeout, tokio::net::TcpStream::connect(addr)).await
            {
                Ok(Ok(_)) => {
                    info!(
                        %addr,
                        elapsed_secs = start.elapsed().as_secs(),
                        "Guest plugin port reachable"
                    );
                    return Ok(());
                }
                Ok(Err(_)) | Err(_) => {}
            }

            if start.elapsed() >= timeout {
                return Err(ResourceError::Timeout(format!(
                    "Guest plugin at {} not reachable within {}s",
                    addr,
                    timeout.as_secs(),
                )));
            }

            warn!(
                %addr,
                elapsed_secs = start.elapsed().as_secs(),
                "Waiting for guest plugin..."
            );
            tokio::time::sleep(interval).await;
            interval = (interval * 2).min(max_interval);
        }
    }
}
