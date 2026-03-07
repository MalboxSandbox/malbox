use crate::{LibvirtError, LibvirtProvider, domain_xml::Domain as XmlDomain};
use async_trait::async_trait;
use malbox_machinery::{Allocate, Machine, MachineEndpoint, MachineId, MachineSpec, MachineState};
use std::error::Error;
use virt::domain::Domain;

#[async_trait]
impl Allocate for LibvirtProvider {
    async fn allocate(&self, spec: &MachineSpec) -> Result<Machine, Box<dyn Error + Send + Sync>> {
        let id = uuid::Uuid::new_v4().to_string();
        let domain_name = format!("malbox-{}", id);

        // Create disk volume
        let disk_path = self
            .create_disk(&domain_name, &spec.storage, spec.base_image.as_deref())
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Build domain XML
        let xml = self
            .build_domain_xml(&domain_name, spec, &disk_path)
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Define the domain in libvirt
        let domain = Domain::define_xml(self.connection(), &xml)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Store machine spec as domain metadata
        let metadata =
            serde_json::to_string(spec).map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        domain
            .set_metadata(
                virt::sys::VIR_DOMAIN_METADATA_DESCRIPTION as i32,
                Some(&metadata),
                None,
                None,
                0,
            )
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Start the domain
        domain
            .create()
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Track allocation
        self.allocated()
            .write()
            .unwrap()
            .insert(MachineId(id.clone()));

        // Create and return Machine
        let mut machine = Machine::new(MachineId(id), spec.clone());
        machine.set_state(MachineState::Running);

        // Try to get network endpoint (might not be available immediately)
        if let Ok(ip) = Self::get_domain_ip(&domain) {
            machine.set_endpoint(Some(malbox_machinery::MachineEndpoint {
                address: ip,
                id: machine.id().to_string(),
                platform: spec.platform,
            }));
        }

        Ok(machine)
    }

    async fn deallocate(&self, machine: &Machine) -> Result<(), Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", machine.id());

        let domain = Domain::lookup_by_name(self.connection(), &domain_name)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // 1. Stop the domain if it's running
        if domain
            .is_active()
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?
        {
            // Try graceful shutdown first
            if domain.shutdown().is_ok() {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }

            // Force destroy if still running
            if domain.is_active().unwrap_or(false) {
                domain
                    .destroy()
                    .map_err(|e| LibvirtError::from(e))
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;
            }
        }

        // 2. Get disk path from domain XML before undefining
        let xml_desc = domain
            .get_xml_desc(0)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let domain_info = XmlDomain::from_xml(&xml_desc).map_err(|e| {
            Box::new(LibvirtError::Libvirt(format!(
                "Failed to parse domain XML: {}",
                e
            ))) as Box<dyn Error + Send + Sync>
        })?;
        let disk_path = domain_info.disk_path();

        // 3. Delete all snapshots for this domain
        if let Ok(snapshots) = domain.list_all_snapshots(0) {
            for snapshot in snapshots {
                let _ = snapshot.delete(0);
            }
        }

        // 4. Undefine (delete) the domain configuration
        domain
            .undefine()
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // 5. Delete disk volume
        if let Some(path) = disk_path {
            if let Ok(vol) = virt::storage_vol::StorageVol::lookup_by_path(self.connection(), &path)
            {
                let _ = vol.delete(0);
            }
        }

        // 6. Remove from allocated set
        self.allocated().write().unwrap().remove(machine.id());

        Ok(())
    }

    async fn list(&self) -> Result<Vec<Machine>, Box<dyn Error + Send + Sync>> {
        // Flags: 0 means list all domains (running, stopped, etc.)
        let domains = self
            .connection()
            .list_all_domains(0)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let mut machines = Vec::new();

        for domain in domains {
            let name = domain
                .get_name()
                .map_err(|e| LibvirtError::from(e))
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

            // Only include domains managed by malbox
            if !name.starts_with("malbox-") {
                continue;
            }

            // Extract machine ID from domain name
            let id = name.strip_prefix("malbox-").unwrap().to_string();

            // Get domain state
            let (state_code, _reason) = domain
                .get_state()
                .map_err(|e| LibvirtError::from(e))
                .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

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
                    .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?,
                Err(_) => {
                    // Skip domains without proper metadata
                    continue;
                }
            };

            let mut machine = Machine::new(MachineId(id), spec);
            machine.set_state(state);

            // Try to get network endpoint
            if matches!(state, MachineState::Running) {
                if let Ok(ip) = Self::get_domain_ip(&domain) {
                    machine.set_endpoint(Some(malbox_machinery::MachineEndpoint {
                        address: ip,
                        id: machine.id().to_string(),
                        platform: machine.spec().platform,
                    }));
                }
            }

            machines.push(machine);
        }

        Ok(machines)
    }

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
                platform: machine.spec().platform,
            })),
            Err(_) => Ok(None),
        }
    }
}

impl LibvirtProvider {
    /// Get IP address from a domain via DHCP lease lookup.
    ///
    /// This is the default strategy for endpoint resolution during polling.
    /// It requires a libvirt-managed DHCP network (e.g. the default NAT network)
    /// and does not need anything installed inside the guest.
    pub(crate) fn get_domain_ip(domain: &Domain) -> Result<std::net::IpAddr, LibvirtError> {
        Self::ip_from_dhcp_lease(domain)
    }

    /// Resolve IP via DHCP lease. Silent when no lease exists yet.
    fn ip_from_dhcp_lease(domain: &Domain) -> Result<std::net::IpAddr, LibvirtError> {
        if let Ok(interfaces) = domain.interface_addresses(
            virt::sys::VIR_DOMAIN_INTERFACE_ADDRESSES_SRC_LEASE as u32,
            0,
        ) {
            for iface in &interfaces {
                for addr in &iface.addrs {
                    if let Ok(ip) = addr.addr.parse::<std::net::IpAddr>() {
                        if !ip.is_loopback() {
                            return Ok(ip);
                        }
                    }
                }
            }
        }

        Err(LibvirtError::Libvirt(
            "No DHCP lease available yet".to_string(),
        ))
    }

    /// Resolve IP via QEMU guest agent. Only works if qemu-guest-agent is
    /// installed and running inside the VM.
    #[allow(dead_code)]
    fn ip_from_guest_agent(domain: &Domain) -> Result<std::net::IpAddr, LibvirtError> {
        if let Ok(interfaces) = domain.interface_addresses(
            virt::sys::VIR_DOMAIN_INTERFACE_ADDRESSES_SRC_AGENT as u32,
            0,
        ) {
            for iface in &interfaces {
                for addr in &iface.addrs {
                    if let Ok(ip) = addr.addr.parse::<std::net::IpAddr>() {
                        if !ip.is_loopback() {
                            return Ok(ip);
                        }
                    }
                }
            }
        }

        Err(LibvirtError::Libvirt(
            "Guest agent not available".to_string(),
        ))
    }
}
