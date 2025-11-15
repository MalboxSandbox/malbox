use async_trait::async_trait;
use malbox_machinery::{
    machine::{MachineId, MachineSpec, MachineState, Storage},
    provider::{
        Provider,
        extension::{ProviderExtension, SnapshotId, SnapshotInfo, Snapshots},
    },
};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use virt::{
    connect::Connect, domain::Domain, domain_snapshot::DomainSnapshot, storage_pool::StoragePool,
};

mod domain_xml;
mod error;
mod machine;
mod snapshot_xml;

use domain_xml::Domain as XmlDomain;
use error::LibvirtError;
use machine::LibvirtMachine;
use snapshot_xml::SnapshotXml;

/// Libvirt provider.
pub struct LibvirtProvider {
    conn: Arc<Connect>,
    config: LibvirtConfig,
    storage_pool: Arc<StoragePool>,
    allocated: Arc<RwLock<HashSet<MachineId>>>,
}

pub struct LibvirtConfig {
    pub uri: String,
    pub storage_pool: String,
}

#[async_trait]
impl Provider for LibvirtProvider {
    type Config = LibvirtConfig;
    type Machine = LibvirtMachine;
    type Error = LibvirtError;

    async fn initialize(config: Self::Config) -> Result<Self, Self::Error> {
        let conn = Arc::new(Connect::open(Some(&config.uri))?);

        let storage_pool = StoragePool::lookup_by_name(&conn, &config.storage_pool)?;

        if !storage_pool.is_active()? {
            // Flags: https://libvirt.org/html/libvirt-libvirt-storage.html#virStoragePoolCreateFlags
            // Flag `0`: Create the pool but do not perform pool build
            storage_pool.create(0)?;
        }

        let mut allocated = HashSet::new();
        // Flags: https://libvirt.org/html/libvirt-libvirt-domain.html#virConnectListAllDomainsFlags
        let domains = conn.list_all_domains(0)?;

        for domain in domains {
            let name = domain.get_name()?;

            if name.starts_with("malbox-") {
                let id = MachineId(name.strip_prefix("malbox-").unwrap().to_string());
                allocated.insert(id);
            }
        }

        Ok(Self {
            conn,
            config,
            storage_pool: Arc::new(storage_pool),
            allocated: Arc::new(RwLock::new(allocated)),
        })
    }

    async fn allocate(&self, spec: MachineSpec) -> Result<Self::Machine, Self::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let domain_name = format!("malbox-{}", id);

        let disk_path = self
            .create_disk(&domain_name, &spec.storage, spec.base_image.as_deref())
            .await?;

        let xml = self.build_domain_xml(&domain_name, &spec, &disk_path)?;
        let domain = Domain::define_xml(&self.conn, &xml)?;

        let metadata = serde_json::to_string(&spec)?;
        domain.set_metadata(
            virt::sys::VIR_DOMAIN_METADATA_DESCRIPTION as i32,
            Some(&metadata),
            None,
            None,
            0,
        )?;

        domain.create()?;

        self.allocated
            .write()
            .unwrap()
            .insert(MachineId(id.clone()));

        Ok(LibvirtMachine {
            id: MachineId(id),
            domain_name,
            spec,
            state: MachineState::Running,
            conn: Arc::clone(&self.conn),
        })
    }

    async fn list(&self) -> Result<Option<Vec<Self::Machine>>, Self::Error> {
        // Flags: 0 means list all domains (running, stopped, etc.)
        let domains = self.conn.list_all_domains(0)?;

        let mut machines = Vec::new();

        for domain in domains {
            let name = domain.get_name()?;

            // Only include domains managed by malbox
            if !name.starts_with("malbox-") {
                continue;
            }

            // Extract machine ID from domain name
            let id = name.strip_prefix("malbox-").unwrap().to_string();

            // Get domain state
            let (state_code, _reason) = domain.get_state()?;
            let state = match state_code {
                virt::sys::VIR_DOMAIN_RUNNING => MachineState::Running,
                virt::sys::VIR_DOMAIN_PAUSED => MachineState::Suspended,
                virt::sys::VIR_DOMAIN_SHUTOFF => MachineState::Stopped,
                virt::sys::VIR_DOMAIN_CRASHED => MachineState::Failed,
                virt::sys::VIR_DOMAIN_SHUTDOWN => MachineState::Stopping,
                _ => MachineState::Stopped,
            };

            // Retrieve machine spec from domain metadata
            let metadata =
                domain.get_metadata(virt::sys::VIR_DOMAIN_METADATA_DESCRIPTION as i32, None, 0);

            let spec = match metadata {
                Ok(json_str) => serde_json::from_str::<MachineSpec>(&json_str)
                    .map_err(|e| LibvirtError::Json(e))?,
                Err(_) => {
                    // If metadata is missing, skip this domain or create a default spec
                    // For now, we'll skip domains without proper metadata
                    continue;
                }
            };

            machines.push(LibvirtMachine {
                id: MachineId(id),
                domain_name: name,
                spec,
                state,
                conn: Arc::clone(&self.conn),
            });
        }

        if machines.is_empty() {
            Ok(None)
        } else {
            Ok(Some(machines))
        }
    }

    async fn deallocate(&self, machine: &Self::Machine) -> Result<(), Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &machine.domain_name)?;

        // 1. Stop the domain if it's running
        if domain.is_active()? {
            // Try graceful shutdown first
            if domain.shutdown().is_ok() {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }

            // Force destroy if still running
            if domain.is_active().unwrap_or(false) {
                domain.destroy()?;
            }
        }

        // 2. Get disk path from domain XML before undefining
        let xml_desc = domain.get_xml_desc(0)?;
        let domain_info = XmlDomain::from_xml(&xml_desc)
            .map_err(|e| LibvirtError::Libvirt(format!("Failed to parse domain XML: {}", e)))?;
        let disk_path = domain_info.disk_path();

        // 3. Delete all snapshots for this domain
        if let Ok(snapshots) = domain.list_all_snapshots(0) {
            for snapshot in snapshots {
                // Ignore errors when deleting snapshots
                let _ = snapshot.delete(0);
            }
        }

        // 4. Undefine (delete) the domain configuration
        domain.undefine()?;

        // 5. Delete disk volume
        if let Some(path) = disk_path {
            if let Ok(vol) = virt::storage_vol::StorageVol::lookup_by_path(&self.conn, &path) {
                let _ = vol.delete(0); // Ignore errors, disk might be shared or already deleted
            }
        }

        // 6. Remove from allocated set
        self.allocated.write().unwrap().remove(&machine.id);

        Ok(())
    }
}

/// Implement the marker trait to indicate this provider supports extensions.
impl ProviderExtension for LibvirtProvider {}

/// Implement snapshot capabilities for LibvirtProvider.
#[async_trait]
impl Snapshots for LibvirtProvider {
    async fn create_snapshot(
        &self,
        machine: &Self::Machine,
        name: &str,
    ) -> Result<SnapshotId, Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &machine.domain_name)?;

        // Create snapshot XML with metadata
        let snapshot_xml = format!(
            r#"<domainsnapshot>
  <name>{}</name>
  <description>Malbox snapshot created at {}</description>
</domainsnapshot>"#,
            name,
            chrono::Utc::now().to_rfc3339()
        );

        // Flags: 0 - VIR_DOMAIN_SNAPSHOT_CREATE_ATOMIC ensures atomicity
        let snapshot = DomainSnapshot::create_xml(&domain, &snapshot_xml, 0)?;
        let snapshot_name = snapshot.get_name()?;

        Ok(SnapshotId(snapshot_name))
    }

    async fn restore_snapshot(
        &self,
        machine: &mut Self::Machine,
        snapshot_id: &SnapshotId,
    ) -> Result<(), Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &machine.domain_name)?;

        // Lookup the snapshot by name
        let snapshot = DomainSnapshot::lookup_by_name(&domain, &snapshot_id.0, 0)?;

        // Revert to the snapshot
        // Flags: 0 means default revert behavior
        DomainSnapshot::revert(&snapshot, 0)?;

        // Update machine state based on domain state after revert
        let (state_code, _) = domain.get_state()?;
        machine.state = match state_code {
            virt::sys::VIR_DOMAIN_RUNNING => MachineState::Running,
            virt::sys::VIR_DOMAIN_PAUSED => MachineState::Suspended,
            virt::sys::VIR_DOMAIN_SHUTOFF => MachineState::Stopped,
            _ => MachineState::Stopped,
        };

        Ok(())
    }

    async fn delete_snapshot(&self, snapshot_id: &SnapshotId) -> Result<(), Self::Error> {
        // Need to find which domain this snapshot belongs to
        // List all malbox domains and check their snapshots
        let domains = self.conn.list_all_domains(0)?;

        for domain in domains {
            let name = domain.get_name()?;
            if !name.starts_with("malbox-") {
                continue;
            }

            // Try to lookup the snapshot in this domain
            if let Ok(snapshot) = DomainSnapshot::lookup_by_name(&domain, &snapshot_id.0, 0) {
                // Delete the snapshot
                // Flags: 0 means default delete behavior
                snapshot.delete(0)?;
                return Ok(());
            }
        }

        Err(LibvirtError::Libvirt(format!(
            "Snapshot '{}' not found in any malbox domain",
            snapshot_id.0
        )))
    }

    async fn list_snapshots(
        &self,
        machine: &Self::Machine,
    ) -> Result<Vec<SnapshotInfo>, Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &machine.domain_name)?;

        // List all snapshots for this domain
        // Flags: 0 means list all snapshots
        let snapshots = domain.list_all_snapshots(0)?;

        let mut snapshot_infos = Vec::new();

        for snapshot in snapshots {
            let name = snapshot.get_name()?;

            // Get snapshot XML and parse it declaratively
            let xml_desc = snapshot.get_xml_desc(0)?;
            let snapshot_data = SnapshotXml::from_xml(&xml_desc).map_err(|e| {
                LibvirtError::Libvirt(format!("Failed to parse snapshot XML: {}", e))
            })?;

            // Check if this is the current snapshot
            let is_current = DomainSnapshot::current(&domain, 0)
                .ok()
                .and_then(|current| current.get_name().ok())
                .map(|current_name| current_name == name)
                .unwrap_or(false);

            snapshot_infos.push(SnapshotInfo {
                id: SnapshotId(name.clone()),
                name,
                description: snapshot_data.description,
                created_at: snapshot_data.creationTime.unwrap_or(0),
                size_bytes: None, // Would need to query disk backing files
                is_current,
            });
        }

        Ok(snapshot_infos)
    }
}

/// Helper methods for LibvirtProvider.
impl LibvirtProvider {
    async fn create_disk(
        &self,
        name: &str,
        storage: &Storage,
        base_image: Option<&str>,
    ) -> Result<String, LibvirtError> {
        use malbox_machinery::machine::DiskType;
        use virt::storage_vol::StorageVol;

        // Determine disk format
        let format = match storage.disk_type {
            DiskType::Qcow2 => "qcow2",
            DiskType::Raw => "raw",
            DiskType::Vmdk => "vmdk",
        };

        let vol_name = format!("{}.{}", name, format);

        if let Some(base_image_path) = base_image {
            // Clone from base image using qcow2 backing file
            if !matches!(storage.disk_type, DiskType::Qcow2) {
                return Err(LibvirtError::Libvirt(
                    "Base image cloning only supported for qcow2 format".to_string(),
                ));
            }

            // Create qcow2 overlay with backing file
            let vol_xml = format!(
                r#"<volume>
  <name>{}</name>
  <capacity unit='bytes'>{}</capacity>
  <target>
    <format type='qcow2'/>
  </target>
  <backingStore>
    <path>{}</path>
    <format type='qcow2'/>
  </backingStore>
</volume>"#,
                vol_name,
                (storage.boot_disk_gb as u64) * 1024 * 1024 * 1024,
                base_image_path
            );

            let vol = StorageVol::create_xml(&self.storage_pool, &vol_xml, 0)?;
            let path = vol.get_path()?;
            Ok(path)
        } else {
            // Create blank disk
            let capacity_bytes = (storage.boot_disk_gb as u64) * 1024 * 1024 * 1024;

            let vol_xml = format!(
                r#"<volume>
  <name>{}</name>
  <capacity unit='bytes'>{}</capacity>
  <target>
    <format type='{}'/>
  </target>
</volume>"#,
                vol_name, capacity_bytes, format
            );

            let vol = StorageVol::create_xml(&self.storage_pool, &vol_xml, 0)?;
            let path = vol.get_path()?;
            Ok(path)
        }
    }

    fn build_domain_xml(
        &self,
        name: &str,
        spec: &MachineSpec,
        disk_path: &str,
    ) -> Result<String, LibvirtError> {
        let domain = XmlDomain::from_spec(name.to_string(), spec, disk_path.to_string());
        domain
            .to_xml()
            .map_err(|e| LibvirtError::Libvirt(format!("XML serialization error: {}", e)))
    }
}
