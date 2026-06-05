//! Provisioning: allocate VM, wait for endpoint, run provisioning steps, snapshot, mark ready.

use crate::error::{ResourceError, Result};
use malbox_config::MachineConfig;
use malbox_database::repositories::machinery::{self, MachinePlatform, MachineStatusDb};
use malbox_database::repositories::provision_runs::{self, ProvisionRun};
use malbox_database::repositories::snapshots;
use malbox_machinery::Machine;
use malbox_machinery::machine::CreateMachineParams;
use malbox_machinery::provider::capabilities::Allocate;
use malbox_machinery::provisioner::{ProvisionContext, ProvisionStatus, create_provisioner};
use sha2::{Digest, Sha256};
use std::net::{IpAddr, SocketAddr};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tracing::{info, instrument, warn};

/// Compute a deterministic hash of a provider_config value.
pub(crate) fn hash_provider_config(config: &Option<toml::Value>) -> String {
    match config {
        Some(val) => {
            let json = serde_json::to_string(val).unwrap_or_default();
            let hash = Sha256::digest(json.as_bytes());
            hex::encode(hash)
        }
        None => "none".to_string(),
    }
}

use super::MachinePool;

/// Name of the base snapshot created during initial machine provisioning.
/// This snapshot represents the clean OS state and is always preserved.
pub const BASE_SNAPSHOT_NAME: &str = "base";

/// Build provider creation parameters from a config-declared machine definition.
fn build_create_params(config: &MachineConfig, image_path: &str) -> CreateMachineParams {
    let platform = match config.platform {
        malbox_config::types::Platform::Windows => malbox_machinery::Platform::Windows,
        malbox_config::types::Platform::Linux => malbox_machinery::Platform::Linux,
    };

    let arch = match config.arch {
        malbox_config::Arch::X64 => malbox_machinery::Arch::X64,
        malbox_config::Arch::X86 => malbox_machinery::Arch::X86,
    };

    CreateMachineParams {
        name: config.name.clone(),
        platform,
        arch,
        cpus: config.cpus,
        memory_mb: config.memory,
        disk_size_mb: config.disk_size,
        base_image: if image_path.is_empty() {
            None
        } else {
            Some(image_path.to_string())
        },
        provider_config: config.provider_config.clone(),
    }
}

/// Build provider creation parameters from DB record (for revert path).
fn build_create_params_from_db(
    db_machine: &machinery::Machine,
    image_path: &str,
) -> CreateMachineParams {
    let platform = match db_machine.platform {
        MachinePlatform::Windows => malbox_machinery::Platform::Windows,
        MachinePlatform::Linux => malbox_machinery::Platform::Linux,
    };
    let arch = match db_machine.arch {
        machinery::MachineArch::X64 => malbox_machinery::Arch::X64,
        machinery::MachineArch::X86 => malbox_machinery::Arch::X86,
    };

    CreateMachineParams {
        name: db_machine.name.clone(),
        platform,
        arch,
        cpus: db_machine.cpus.unwrap_or(2) as u32,
        memory_mb: db_machine.memory_mb.unwrap_or(2048) as u32,
        disk_size_mb: db_machine.disk_size_mb.unwrap_or(65536) as u64,
        base_image: if image_path.is_empty() {
            None
        } else {
            Some(image_path.to_string())
        },
        provider_config: None,
    }
}

impl MachinePool {
    /// Create a machine from an image and take a base snapshot.
    #[instrument(skip_all, fields(machine_id), err)]
    pub(crate) async fn setup_machine(&self, machine_id: i32, image_path: &str) -> Result<()> {
        let allocate = self.provider.allocate();

        let db_machine = machinery::fetch_machine(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or(ResourceError::MachineNotFound {
                id: machine_id.to_string(),
            })?;

        machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Creating, None)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        let params = build_create_params_from_db(&db_machine, image_path);
        let mut machine = allocate
            .create(&params)
            .await
            .map_err(|e| ResourceError::AllocationFailed(e.to_string()))?;

        // Start temporarily to get an IP via DHCP
        allocate
            .start(&machine)
            .await
            .map_err(|e| ResourceError::AllocationFailed(e.to_string()))?;

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

        machinery::set_machine_provider_info(
            &self.db,
            machine_id,
            self.provider.name(),
            &machine.id.0,
            &ip_str,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        // Stop before snapshotting — produces an internal disk-only snapshot
        allocate
            .stop(&machine, true)
            .await
            .map_err(|e| ResourceError::Provider(format!("Failed to stop VM: {}", e)))?;

        // Create the "base" snapshot
        if let Some(snapshot_cap) = self.provider.snapshot() {
            let snap_id = snapshot_cap
                .create_snapshot(&machine, BASE_SNAPSHOT_NAME)
                .await
                .map_err(|e| ResourceError::Provider(e.to_string()))?;

            snapshots::insert_snapshot(
                &self.db,
                machine_id,
                BASE_SNAPSHOT_NAME,
                &snap_id.0,
                Some("Clean OS disk state"),
                None,
                None,
                true,
            )
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

            info!(
                machine_id,
                provider_id = %snap_id,
                "Base snapshot created (disk-only)"
            );
        }

        machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Ready, None)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        self.machine_available.notify_waiters();
        info!(machine_id, "Machine is ready");

        Ok(())
    }

    /// Create a machine from a config definition: allocate, wait for endpoint, base snapshot.
    /// Does NOT mark the machine Ready — the caller is responsible for that.
    #[instrument(skip_all, fields(machine_id), err)]
    pub(crate) async fn setup_machine_from_config(
        &self,
        machine_id: i32,
        image_path: &str,
        config: &MachineConfig,
    ) -> Result<()> {
        let allocate = self.provider.allocate();

        machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Creating, None)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

        let params = build_create_params(config, image_path);
        let mut machine = allocate
            .create(&params)
            .await
            .map_err(|e| ResourceError::AllocationFailed(e.to_string()))?;

        allocate
            .start(&machine)
            .await
            .map_err(|e| ResourceError::AllocationFailed(e.to_string()))?;

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

        machinery::set_machine_provider_info(
            &self.db,
            machine_id,
            self.provider.name(),
            &machine.id.0,
            &ip_str,
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        allocate
            .stop(&machine, true)
            .await
            .map_err(|e| ResourceError::Provider(format!("Failed to stop VM: {}", e)))?;

        if let Some(snapshot_cap) = self.provider.snapshot() {
            let snap_id = snapshot_cap
                .create_snapshot(&machine, BASE_SNAPSHOT_NAME)
                .await
                .map_err(|e| ResourceError::Provider(e.to_string()))?;

            snapshots::insert_snapshot(
                &self.db,
                machine_id,
                BASE_SNAPSHOT_NAME,
                &snap_id.0,
                Some("Clean OS disk state"),
                None,
                None,
                true,
            )
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?;

            info!(
                machine_id,
                provider_id = %snap_id,
                "Base snapshot created (disk-only)"
            );
        }

        Ok(())
    }

    /// Run a provisioning step against a ready machine.
    ///
    /// Returns the completed ProvisionRun record.
    #[instrument(skip_all, fields(machine_id, step = %step.provisioner_type), err)]
    pub async fn run_provision_step(
        &self,
        machine_id: i32,
        step: &malbox_config::provisioning::ProvisionStep,
        revert_to: Option<&str>,
    ) -> Result<ProvisionRun> {
        // Atomic Ready → Provisioning transition (prevents concurrent provisioning)
        let db_machine = machinery::transition_to_provisioning(&self.db, machine_id)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))?
            .ok_or(ResourceError::InvalidMachineState {
                id: machine_id,
                status: "not ready".to_string(),
                expected: "ready".to_string(),
            })?;

        // If the step creates a snapshot, check the name isn't already taken in the DB.
        if let Some(ref snap_name) = step.snapshot {
            let existing = snapshots::fetch_snapshots_for_machine(&self.db, machine_id)
                .await
                .map_err(|e| ResourceError::Database(e.to_string()))?;

            if existing.iter().any(|s| s.name == *snap_name) {
                let _ = machinery::update_machine_status(
                    &self.db,
                    machine_id,
                    MachineStatusDb::Ready,
                    None,
                )
                .await;
                return Err(ResourceError::Internal(format!(
                    "Snapshot '{}' already exists on machine {}",
                    snap_name, machine_id
                )));
            }
        }

        // Validate preconditions before creating the provision run
        let provider_id = match db_machine.provider_id.as_deref() {
            Some(id) if !id.is_empty() => id.to_string(),
            _ => {
                let _ = machinery::update_machine_status(
                    &self.db,
                    machine_id,
                    MachineStatusDb::Ready,
                    None,
                )
                .await;
                return Err(ResourceError::MachineNotReady {
                    reason: "Machine has no provider_id".to_string(),
                });
            }
        };

        let ip: std::net::IpAddr = match db_machine.ip.as_deref() {
            Some(ip_str) => match ip_str.parse() {
                Ok(ip) => ip,
                Err(e) => {
                    let _ = machinery::update_machine_status(
                        &self.db,
                        machine_id,
                        MachineStatusDb::Ready,
                        None,
                    )
                    .await;
                    return Err(ResourceError::Internal(format!("Invalid IP in DB: {}", e)));
                }
            },
            None => {
                let _ = machinery::update_machine_status(
                    &self.db,
                    machine_id,
                    MachineStatusDb::Ready,
                    None,
                )
                .await;
                return Err(ResourceError::MachineNotReady {
                    reason: "Machine has no IP address".to_string(),
                });
            }
        };

        let platform = match db_machine.platform {
            MachinePlatform::Windows => malbox_machinery::Platform::Windows,
            MachinePlatform::Linux => malbox_machinery::Platform::Linux,
        };

        // Convert step config to JSON for storage
        let config_json = serde_json::to_value(&step.config).ok();

        // Insert provision run record (after validation passes)
        let run = provision_runs::insert_provision_run(
            &self.db,
            machine_id,
            &step.provisioner_type,
            config_json.as_ref(),
        )
        .await
        .map_err(|e| ResourceError::Database(e.to_string()))?;

        // Run the provisioner
        let result = self
            .do_provision_step(machine_id, &provider_id, ip, platform, step, revert_to)
            .await;

        let completed_run = match result {
            Ok(()) => {
                // Look up the snapshot by name (if one was requested)
                let snapshot_id = if let Some(ref snap_name) = step.snapshot {
                    snapshots::fetch_snapshots_for_machine(&self.db, machine_id)
                        .await
                        .ok()
                        .and_then(|snaps| snaps.iter().find(|s| s.name == *snap_name).map(|s| s.id))
                } else {
                    None
                };

                provision_runs::update_provision_run_success(&self.db, run.id, None, snapshot_id)
                    .await
                    .map_err(|e| ResourceError::Database(e.to_string()))?
            }
            Err(ref e) => {
                let output_json = serde_json::to_value(e.to_string()).ok();
                provision_runs::update_provision_run_failed(
                    &self.db,
                    run.id,
                    &e.to_string(),
                    output_json.as_ref(),
                )
                .await
                .map_err(|e| ResourceError::Database(e.to_string()))?
            }
        };

        // Always restore machine to Ready
        let _ =
            machinery::update_machine_status(&self.db, machine_id, MachineStatusDb::Ready, None)
                .await;

        info!(machine_id, run_id = %completed_run.id, status = ?completed_run.status, "Provisioning step complete");
        Ok(completed_run)
    }

    /// Inner provisioning logic.
    ///
    /// Reverts to the specified snapshot (or "base" by default), starts the VM,
    /// waits for the management service, runs the provisioner, and snapshots.
    async fn do_provision_step(
        &self,
        machine_id: i32,
        provider_id: &str,
        ip: std::net::IpAddr,
        platform: malbox_machinery::Platform,
        step: &malbox_config::provisioning::ProvisionStep,
        revert_to: Option<&str>,
    ) -> Result<()> {
        let allocate = self.provider.allocate();
        let provider_vms = allocate
            .list()
            .await
            .map_err(|e| ResourceError::Provider(e.to_string()))?;

        let runtime_vm = provider_vms
            .iter()
            .find(|vm| vm.id.0 == provider_id)
            .ok_or_else(|| ResourceError::MachineNotFound {
                id: provider_id.to_string(),
            })?;

        // Validate provisioner config early — before starting the VM.
        // This catches errors like missing playbook paths without the cost
        // of booting the VM and waiting for connectivity.
        let provisioner = create_provisioner(&step.provisioner_type, &step.config)
            .map_err(|e| ResourceError::Provisioner(e.to_string()))?;

        // Clean up stale provider snapshots that aren't tracked in the DB
        // (e.g. from a previous DB-only delete) before doing any work.
        if let Some(ref snap_name) = step.snapshot
            && let Some(snapshot_cap) = self.provider.snapshot()
            && let Ok(provider_snaps) = snapshot_cap.list_snapshots(runtime_vm).await
        {
            let db_snaps = snapshots::fetch_snapshots_for_machine(&self.db, machine_id)
                .await
                .unwrap_or_default();

            if let Some(stale) = provider_snaps.iter().find(|ps| {
                ps.name == *snap_name
                    && !db_snaps.iter().any(|ds| ds.provider_snapshot_id == ps.id.0)
            }) {
                warn!(
                    machine_id,
                    snapshot = snap_name,
                    "Stale provider snapshot found (not in DB), deleting before provisioning"
                );
                snapshot_cap.delete_snapshot(&stale.id).await.map_err(|e| {
                    ResourceError::Provider(format!(
                        "Failed to delete stale provider snapshot '{}': {}",
                        snap_name, e
                    ))
                })?;
            }
        }

        // Revert to a snapshot so we provision from a clean state
        let revert_name = revert_to.unwrap_or(BASE_SNAPSHOT_NAME);

        if let Some(snapshot_cap) = self.provider.snapshot() {
            let snap = snapshots::fetch_snapshots_for_machine(&self.db, machine_id)
                .await
                .map_err(|e| ResourceError::Database(e.to_string()))?
                .into_iter()
                .find(|s| s.name == revert_name);

            match snap {
                Some(snap) => {
                    info!(
                        machine_id,
                        snapshot = revert_name,
                        "Reverting to snapshot before provisioning"
                    );
                    snapshot_cap
                        .restore_snapshot(
                            runtime_vm,
                            &malbox_machinery::SnapshotId(snap.provider_snapshot_id),
                        )
                        .await
                        .map_err(|e| {
                            ResourceError::Provider(format!("Snapshot revert failed: {}", e))
                        })?;
                }
                None => {
                    return Err(ResourceError::Internal(format!(
                        "Snapshot '{}' not found on machine {}",
                        revert_name, machine_id
                    )));
                }
            }
        }

        // Start the VM (snapshot revert may leave it running or stopped)
        allocate
            .start(runtime_vm)
            .await
            .map_err(|e| ResourceError::Provider(format!("Failed to start VM: {}", e)))?;

        // Wait for the management service
        let mgmt_port = match platform {
            malbox_machinery::Platform::Windows => 5986,
            malbox_machinery::Platform::Linux => 22,
        };

        info!(
            machine_id,
            %ip, port = mgmt_port,
            "Waiting for management service"
        );

        wait_for_connectivity(ip, mgmt_port, std::time::Duration::from_secs(180)).await?;

        let endpoint = malbox_machinery::machine::MachineEndpoint {
            address: ip,
            id: provider_id.to_string(),
            platform,
        };

        let context = ProvisionContext {
            endpoint,
            config: step.config.clone(),
        };

        info!(
            machine_id,
            provisioner = provisioner.name(),
            snapshot = ?step.snapshot,
            "Running provisioning step"
        );

        let result = provisioner
            .provision(&context)
            .await
            .map_err(|e| ResourceError::Provisioner(e.to_string()))?;

        if result.status == ProvisionStatus::Failed {
            return Err(ResourceError::Provisioner(format!(
                "Step '{}' failed: {}",
                provisioner.name(),
                result.output.as_deref().unwrap_or("no output"),
            )));
        }

        // Stop the VM before snapshotting — produces an internal disk-only
        // snapshot. Reverts restore the disk cleanly and the VM boots fresh.
        allocate
            .stop(runtime_vm, false)
            .await
            .map_err(|e| ResourceError::Provider(format!("Failed to stop VM: {}", e)))?;

        // Create snapshot if requested
        if let Some(ref snap_name) = step.snapshot {
            if let Some(snapshot_cap) = self.provider.snapshot() {
                let snap_id = snapshot_cap
                    .create_snapshot(runtime_vm, snap_name)
                    .await
                    .map_err(|e| ResourceError::Provider(e.to_string()))?;

                let guest_plugins_json = if step.guest_plugins.is_empty() {
                    None
                } else {
                    Some(serde_json::json!(step.guest_plugins))
                };

                snapshots::insert_snapshot(
                    &self.db,
                    machine_id,
                    snap_name,
                    &snap_id.0,
                    None,
                    None,
                    guest_plugins_json.as_ref(),
                    true,
                )
                .await
                .map_err(|e| ResourceError::Database(e.to_string()))?;

                info!(
                    machine_id,
                    snapshot = snap_name,
                    provider_id = %snap_id,
                    "Snapshot created (disk-only)"
                );
            } else {
                warn!(
                    machine_id,
                    snapshot = snap_name,
                    "Step requests snapshot but provider does not support snapshots"
                );
            }
        }

        Ok(())
    }
}

/// Wait until a machine has an endpoint (IP address) available.
///
/// Polls the provider's `resolve_endpoint` method with exponential backoff.
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
            Ok(None) => {}
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

        interval = (interval * 2).min(max_interval);
    }
}

/// Wait until a TCP connection can be established to the given address and port.
///
/// Used to verify that the VM's management service (WinRM/SSH) is reachable
/// before running a provisioner. Retries with exponential backoff.
async fn wait_for_connectivity(ip: IpAddr, port: u16, timeout: std::time::Duration) -> Result<()> {
    let addr = SocketAddr::new(ip, port);
    let start = Instant::now();
    let mut interval = std::time::Duration::from_secs(5);
    let max_interval = std::time::Duration::from_secs(10);
    let connect_timeout = std::time::Duration::from_secs(10);

    loop {
        match tokio::time::timeout(connect_timeout, TcpStream::connect(addr)).await {
            Ok(Ok(_stream)) => {
                info!(
                    %addr,
                    elapsed_secs = start.elapsed().as_secs(),
                    "Management service is reachable"
                );
                return Ok(());
            }
            Ok(Err(e)) => {
                warn!(%addr, error = %e, "TCP connect failed");
            }
            Err(_) => {
                warn!(%addr, "TCP connect timed out");
            }
        }

        if start.elapsed() >= timeout {
            return Err(ResourceError::Timeout(format!(
                "Management service at {} not reachable within {}s",
                addr,
                timeout.as_secs(),
            )));
        }

        info!(
            %addr,
            elapsed_secs = start.elapsed().as_secs(),
            "Waiting for management service..."
        );

        tokio::time::sleep(interval).await;
        interval = (interval * 2).min(max_interval);
    }
}
