use crate::{LibvirtError, LibvirtProvider, domain_xml::Domain as XmlDomain};
use async_trait::async_trait;
use malbox_machinery::{
    Allocate, CreateMachineParams, Machine, MachineEndpoint, MachineId, MachineState, Platform,
};
use std::error::Error;
use virt::domain::Domain;

#[async_trait]
impl Allocate for LibvirtProvider {
    /// Create a libvirt domain with a COW disk overlay.
    ///
    /// Defines the domain in libvirt but does **not** boot it.
    /// Call [`start`] to boot.
    async fn create(
        &self,
        params: &CreateMachineParams,
    ) -> Result<Machine, Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", params.name);
        let id = params.name.clone();

        // Create a qcow2 overlay disk (backed by the base image if provided)
        let capacity_bytes = params.disk_size_mb * 1024 * 1024;
        let disk_path = self
            .create_disk(&domain_name, params.base_image.as_deref(), capacity_bytes)
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Generate the libvirt domain XML
        let xml = self
            .build_domain_xml(&domain_name, params, &disk_path)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Register the domain in libvirt (persistent, not started)
        let domain = Domain::define_xml(self.connection(), &xml)
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Store the machine name in domain metadata so we can link back
        // to the DB record when reconstructing Machine structs on restart.
        domain
            .set_metadata(
                virt::sys::VIR_DOMAIN_METADATA_DESCRIPTION as i32,
                Some(&params.name),
                None,
                None,
                0,
            )
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        self.allocated()
            .write()
            .unwrap()
            .insert(MachineId(id.clone()));

        // Return the machine in Stopped state — caller must call start()
        let machine = Machine::new(MachineId(id));
        Ok(machine)
    }

    /// Tear down a libvirt domain and all its resources.
    ///
    /// Force-stops the domain if running, deletes all snapshots, removes the
    /// domain definition, and deletes the disk volume.
    async fn destroy(&self, machine: &Machine) -> Result<(), Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", machine.id());

        let domain = Domain::lookup_by_name(self.connection(), &domain_name)
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Force-stop if running
        if domain.is_active().unwrap_or(false) {
            let _ = domain.destroy();
        }

        // Capture the disk path before we remove the domain definition
        let xml_desc = domain
            .get_xml_desc(0)
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let domain_info = XmlDomain::from_xml(&xml_desc).map_err(|e| {
            Box::new(LibvirtError::Libvirt(format!(
                "Failed to parse domain XML: {}",
                e
            ))) as Box<dyn Error + Send + Sync>
        })?;
        let disk_path = domain_info.disk_path();

        // Remove all libvirt snapshots
        if let Ok(snapshots) = domain.list_all_snapshots(0) {
            for snapshot in snapshots {
                let _ = snapshot.delete(0);
            }
        }

        // Remove the domain definition from libvirt
        domain
            .undefine()
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Delete the disk volume from the storage pool
        if let Some(path) = disk_path
            && let Ok(vol) = virt::storage_vol::StorageVol::lookup_by_path(self.connection(), &path)
        {
            let _ = vol.delete(0);
        }

        self.allocated().write().unwrap().remove(machine.id());
        Ok(())
    }

    /// Boot a stopped libvirt domain.
    ///
    /// Uses `domain.create()` which is libvirt's API for starting a defined
    /// but inactive domain. No-op if already running.
    async fn start(&self, machine: &Machine) -> Result<(), Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", machine.id());

        let domain = Domain::lookup_by_name(self.connection(), &domain_name)
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        if !domain
            .is_active()
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?
        {
            // libvirt calls this "create" — it boots an already-defined domain
            domain
                .create()
                .map_err(LibvirtError::from)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        }

        Ok(())
    }

    /// Shut down a running libvirt domain.
    ///
    /// With `force: false`, sends an ACPI shutdown signal and waits up to 5s
    /// for the guest to power off. Falls back to force-kill if it doesn't.
    /// With `force: true`, kills immediately.
    async fn stop(
        &self,
        machine: &Machine,
        force: bool,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", machine.id());

        let domain = Domain::lookup_by_name(self.connection(), &domain_name)
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        if !domain.is_active().unwrap_or(false) {
            return Ok(());
        }

        if force {
            domain
                .destroy()
                .map_err(LibvirtError::from)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
        } else {
            // Try ACPI shutdown, fall back to force-kill
            if domain.shutdown().is_ok() {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
            if domain.is_active().unwrap_or(false) {
                domain
                    .destroy()
                    .map_err(LibvirtError::from)
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
            }
        }

        Ok(())
    }

    /// List all malbox-managed libvirt domains.
    ///
    /// Filters to domains with the `malbox-` name prefix. The machine name
    /// stored in domain metadata links back to the DB record.
    async fn list(&self) -> Result<Vec<Machine>, Box<dyn Error + Send + Sync>> {
        let domains = self
            .connection()
            .list_all_domains(0)
            .map_err(LibvirtError::from)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let mut machines = Vec::new();

        for domain in domains {
            let name = domain
                .get_name()
                .map_err(LibvirtError::from)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

            // Only include domains managed by malbox
            if !name.starts_with("malbox-") {
                continue;
            }

            let id = name.strip_prefix("malbox-").unwrap().to_string();

            let (state_code, _reason) = domain
                .get_state()
                .map_err(LibvirtError::from)
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

            let state = match state_code {
                virt::sys::VIR_DOMAIN_RUNNING => MachineState::Running,
                virt::sys::VIR_DOMAIN_PAUSED => MachineState::Suspended,
                virt::sys::VIR_DOMAIN_SHUTOFF => MachineState::Stopped,
                virt::sys::VIR_DOMAIN_CRASHED => MachineState::Failed,
                virt::sys::VIR_DOMAIN_SHUTDOWN => MachineState::Stopping,
                _ => MachineState::Stopped,
            };

            let mut machine = Machine::new(MachineId(id));
            machine.set_state(state);

            // Resolve IP if the domain is running
            if matches!(state, MachineState::Running)
                && let Ok(ip) = Self::get_domain_ip(&domain)
            {
                machine.set_endpoint(Some(MachineEndpoint {
                    address: ip,
                    id: machine.id().to_string(),
                    platform: Platform::Windows, // TODO: store platform in domain metadata
                }));
            }

            machines.push(machine);
        }

        Ok(machines)
    }

    /// Resolve IP address for a domain via DHCP lease lookup.
    async fn resolve_endpoint(
        &self,
        machine: &Machine,
    ) -> Result<Option<MachineEndpoint>, Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", machine.id());

        let domain = match Domain::lookup_by_name(self.connection(), &domain_name) {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };

        match Self::get_domain_ip(&domain) {
            Ok(ip) => Ok(Some(MachineEndpoint {
                address: ip,
                id: machine.id().to_string(),
                platform: Platform::Windows, // TODO: store platform in domain metadata
            })),
            Err(_) => Ok(None),
        }
    }
}
