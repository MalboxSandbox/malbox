use crate::{LibvirtError, LibvirtProvider, snapshot_xml::SnapshotXml};
use async_trait::async_trait;
use malbox_machinery::{Machine, Snapshot, SnapshotId, SnapshotInfo};
use std::error::Error;
use virt::{domain::Domain, domain_snapshot::DomainSnapshot};

#[async_trait]
impl Snapshot for LibvirtProvider {
    async fn create_snapshot(
        &self,
        machine: &Machine,
        name: &str,
    ) -> Result<SnapshotId, Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", machine.id());

        let domain =
            Domain::lookup_by_name(self.connection(), &domain_name).map_err(LibvirtError::from)?;

        // Create snapshot XML with metadata
        let snapshot_xml = format!(
            r"<domainsnapshot>
  <name>{}</name>
  <description>Malbox snapshot created at {}</description>
</domainsnapshot>",
            name,
            chrono::Utc::now().to_rfc3339()
        );

        // Flags: 0 - VIR_DOMAIN_SNAPSHOT_CREATE_ATOMIC ensures atomicity
        let snapshot =
            DomainSnapshot::create_xml(&domain, &snapshot_xml, 0).map_err(LibvirtError::from)?;

        let snapshot_name = snapshot.get_name().map_err(LibvirtError::from)?;

        Ok(SnapshotId(snapshot_name))
    }

    async fn restore_snapshot(
        &self,
        machine: &Machine,
        snapshot_id: &SnapshotId,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", machine.id());

        let domain =
            Domain::lookup_by_name(self.connection(), &domain_name).map_err(LibvirtError::from)?;

        // Lookup the snapshot by name
        let snapshot = DomainSnapshot::lookup_by_name(&domain, &snapshot_id.0, 0)
            .map_err(LibvirtError::from)?;

        // Revert to the snapshot
        // Flags: 0 means default revert behavior
        DomainSnapshot::revert(&snapshot, 0).map_err(LibvirtError::from)?;

        Ok(())
    }

    async fn delete_snapshot(
        &self,
        snapshot_id: &SnapshotId,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Need to find which domain this snapshot belongs to
        // List all malbox domains and check their snapshots
        let domains = self
            .connection()
            .list_all_domains(0)
            .map_err(LibvirtError::from)?;

        for domain in domains {
            let name = domain.get_name().map_err(LibvirtError::from)?;

            if !name.starts_with("malbox-") {
                continue;
            }

            // Try to lookup the snapshot in this domain
            if let Ok(snapshot) = DomainSnapshot::lookup_by_name(&domain, &snapshot_id.0, 0) {
                // Delete the snapshot
                // Flags: 0 means default delete behavior
                snapshot.delete(0).map_err(LibvirtError::from)?;
                return Ok(());
            }
        }

        Err(Box::new(LibvirtError::Libvirt(format!(
            "Snapshot '{}' not found in any malbox domain",
            snapshot_id.0
        ))))
    }

    async fn list_snapshots(
        &self,
        machine: &Machine,
    ) -> Result<Vec<SnapshotInfo>, Box<dyn Error + Send + Sync>> {
        let domain_name = format!("malbox-{}", machine.id());

        let domain =
            Domain::lookup_by_name(self.connection(), &domain_name).map_err(LibvirtError::from)?;

        // List all snapshots for this domain
        // Flags: 0 means list all snapshots
        let snapshots = domain.list_all_snapshots(0).map_err(LibvirtError::from)?;

        let mut snapshot_infos = Vec::new();

        for snapshot in snapshots {
            let name = snapshot.get_name().map_err(LibvirtError::from)?;

            // Get snapshot XML and parse it
            let xml_desc = snapshot.get_xml_desc(0).map_err(LibvirtError::from)?;

            let snapshot_data = SnapshotXml::from_xml(&xml_desc)
                .map_err(|e| LibvirtError::Libvirt(format!("Failed to parse snapshot XML: {e}")))?;

            // Check if this is the current snapshot
            let is_current = DomainSnapshot::current(&domain, 0)
                .ok()
                .and_then(|current| current.get_name().ok())
                .is_some_and(|current_name| current_name == name);

            snapshot_infos.push(SnapshotInfo {
                id: SnapshotId(name.clone()),
                name,
                description: snapshot_data.description,
                created_at: snapshot_data.creation_time.unwrap_or(0),
                size_bytes: None, // Would need to query disk backing files
                is_current,
            });
        }

        Ok(snapshot_infos)
    }
}
