use crate::{domain_xml::Domain as XmlDomain, LibvirtError, LibvirtProvider};
use async_trait::async_trait;
use malbox_machinery::{Clone, Machine, MachineId, MachineState};
use std::error::Error;
use virt::domain::Domain;

#[async_trait]
impl Clone for LibvirtProvider {
    async fn clone_machine(
        &self,
        machine: &Machine,
        new_name: &str,
    ) -> Result<Machine, Box<dyn Error + Send + Sync>> {
        let source_domain_name = format!("malbox-{}", machine.id());

        let source_domain = Domain::lookup_by_name(self.connection(), &source_domain_name)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Get the XML of the source domain
        let source_xml = source_domain
            .get_xml_desc(0)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Parse source domain XML
        let mut domain_info = XmlDomain::from_xml(&source_xml).map_err(|e| {
            Box::new(LibvirtError::Libvirt(format!(
                "Failed to parse domain XML: {}",
                e
            ))) as Box<dyn Error + Send + Sync>
        })?;

        // Generate new ID and name for clone
        let id = uuid::Uuid::new_v4().to_string();
        let clone_domain_name = format!("malbox-{}", id);

        // Update domain name in XML
        domain_info.name = clone_domain_name.clone();

        // Clone the disk using qcow2 backing file
        let source_disk_path = domain_info
            .disk_path()
            .ok_or_else(|| {
                Box::new(LibvirtError::Libvirt(
                    "Source domain has no disk".to_string(),
                )) as Box<dyn Error + Send + Sync>
            })?;

        // Create new disk with source as backing file
        let clone_disk_path = self
            .create_disk(
                &clone_domain_name,
                &machine.spec().storage,
                Some(&source_disk_path),
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Update disk path in domain XML
        domain_info.update_disk_path(&clone_disk_path);

        // Convert back to XML
        let clone_xml = domain_info.to_xml().map_err(|e| {
            Box::new(LibvirtError::Libvirt(format!(
                "Failed to serialize domain XML: {}",
                e
            ))) as Box<dyn Error + Send + Sync>
        })?;

        // Define the cloned domain
        let clone_domain = Domain::define_xml(self.connection(), &clone_xml)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Store machine spec as metadata
        let metadata = serde_json::to_string(machine.spec())
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        clone_domain
            .set_metadata(
                virt::sys::VIR_DOMAIN_METADATA_DESCRIPTION as i32,
                Some(&metadata),
                None,
                None,
                0,
            )
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Start the cloned domain
        clone_domain
            .create()
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Track allocation
        self.allocated()
            .write()
            .unwrap()
            .insert(MachineId(id.clone()));

        // Create and return Machine
        let mut clone_machine = Machine::new(MachineId(id), machine.spec().clone());
        clone_machine.set_state(MachineState::Running);

        // Try to get network endpoint
        if let Ok(ip) = LibvirtProvider::get_domain_ip(&clone_domain) {
            clone_machine.set_endpoint(Some(malbox_machinery::MachineEndpoint {
                address: ip,
                id: clone_machine.id().to_string(),
                platform: machine.spec().platform,
            }));
        }

        Ok(clone_machine)
    }
}
