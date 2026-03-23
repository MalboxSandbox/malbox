use crate::{LibvirtError, LibvirtProvider, domain_xml::Domain as XmlDomain};
use async_trait::async_trait;
use malbox_machinery::{Clone, Machine, MachineId};
use std::error::Error;
use virt::domain::Domain;

#[async_trait]
impl Clone for LibvirtProvider {
    /// Clone a machine by creating a new domain with a COW overlay disk
    /// backed by the source domain's disk.
    async fn clone_machine(
        &self,
        machine: &Machine,
        _new_name: &str,
    ) -> Result<Machine, Box<dyn Error + Send + Sync>> {
        let source_domain_name = format!("malbox-{}", machine.id());

        let source_domain = Domain::lookup_by_name(self.connection(), &source_domain_name)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Parse source domain XML to get disk path
        let source_xml = source_domain
            .get_xml_desc(0)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        let mut domain_info = XmlDomain::from_xml(&source_xml).map_err(|e| {
            Box::new(LibvirtError::Libvirt(format!(
                "Failed to parse domain XML: {}",
                e
            ))) as Box<dyn Error + Send + Sync>
        })?;

        // Generate new ID and domain name for the clone
        let id = uuid::Uuid::new_v4().to_string();
        let clone_domain_name = format!("malbox-{}", id);

        domain_info.name = clone_domain_name.clone();

        // Create new disk using source disk as backing file
        let source_disk_path = domain_info.disk_path().ok_or_else(|| {
            Box::new(LibvirtError::Libvirt(
                "Source domain has no disk".to_string(),
            )) as Box<dyn Error + Send + Sync>
        })?;

        let clone_disk_path = self
            .create_disk(
                &clone_domain_name,
                Some(&source_disk_path),
                64 * 1024 * 1024 * 1024,
            )
            .await
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        domain_info.update_disk_path(&clone_disk_path);

        // Define the cloned domain
        let clone_xml = domain_info.to_xml().map_err(|e| {
            Box::new(LibvirtError::Libvirt(format!(
                "Failed to serialize domain XML: {}",
                e
            ))) as Box<dyn Error + Send + Sync>
        })?;

        let _clone_domain = Domain::define_xml(self.connection(), &clone_xml)
            .map_err(|e| LibvirtError::from(e))
            .map_err(|e| Box::new(e) as Box<dyn Error + Send + Sync>)?;

        // Track allocation
        self.allocated()
            .write()
            .unwrap()
            .insert(MachineId(id.clone()));

        // Return stopped — caller must start() explicitly
        let machine = Machine::new(MachineId(id));
        Ok(machine)
    }
}
