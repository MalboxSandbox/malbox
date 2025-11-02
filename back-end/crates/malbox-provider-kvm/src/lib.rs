use async_trait::async_trait;
use malbox_machinery::{
    machine::{Machine, MachineId, MachineSpec, MachineState, Storage},
    provider::{Capabilities, Provider},
};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use virt::{connect::Connect, domain::Domain, storage_pool::StoragePool};

mod error;
mod machine;

use error::KvmError;
use machine::KvmMachine;

/// KVM provider using libvirt.
pub struct KvmProvider {
    conn: Arc<Connect>,
    config: KvmConfig,
    storage_pool: Arc<StoragePool>,
    allocated: Arc<RwLock<HashSet<MachineId>>>,
}

pub struct KvmConfig {
    pub uri: String,
    pub storage_pool: String,
}

#[async_trait]
impl Provider for KvmProvider {
    type Config = KvmConfig;
    type Machine = KvmMachine;
    type Error = KvmError;

    const CAPABILITIES: Capabilities = Capabilities {};

    async fn initialize(config: Self::Config) -> Result<Self, Self::Error> {
        let conn = Arc::new(
            Connect::open(Some(&config.uri))
                .map_err(|e| KvmError::Libvirt(format!("failed to connect: {}", e)))?,
        );

        let storage_pool = StoragePool::lookup_by_name(&conn, &config.storage_pool)
            .map_err(|e| KvmError::Libvirt(format!("Pool not found: {}", e)))?;

        if !storage_pool
            .is_active()
            .map_err(|e| KvmError::Libvirt(e.to_string()))?
        {
            // Flags: https://libvirt.org/html/libvirt-libvirt-storage.html#virStoragePoolCreateFlags
            // Flag `0`: Create the pool but do not perform pool build
            storage_pool
                .create(0)
                .map_err(|e| KvmError::Libvirt(format!("Failed to activate pool: {}", e)))?;
        }

        let mut allocated = HashSet::new();
        // Flags: https://libvirt.org/html/libvirt-libvirt-domain.html#virConnectListAllDomainsFlags
        let domains = conn
            .list_all_domains(0)
            .map_err(|e| KvmError::Libvirt(e.to_string()))?;

        for domain in domains {
            let name = domain
                .get_name()
                .map_err(|e| KvmError::Libvirt(e.to_string()))?;

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

    async fn allocate(&self, mut spec: MachineSpec) -> Result<Self::Machine, Self::Error> {
        // TODO: Actual machineId gen
        let id = String::from("Test");
        let domain_name = format!("malbox-{}", id);

        let disk_path = self.create_disk(&domain_name, &spec.storage).await?;

        let xml = self.build_domain_xml(&domain_name, &spec, &disk_path)?;
        let domain = Domain::define_xml(&self.conn, &xml)
            .map_err(|e| KvmError::Libvirt(format!("Failed to define domain: {}", e)))?;

        let metadata = serde_json::to_string(&spec)
            .map_err(|e| KvmError::Libvirt(format!("Failed to serialize spec: {}", e)))?;
        domain
            .set_metadata(
                virt::sys::VIR_DOMAIN_METADATA_DESCRIPTION as i32,
                Some(&metadata),
                None,
                None,
                0,
            )
            .map_err(|e| KvmError::Libvirt(format!("Failed to set metadata: {}", e)))?;

        domain
            .create()
            .map_err(|e| KvmError::Libvirt(format!("Failed to start domain: {}", e)))?;

        self.allocated
            .write()
            .unwrap()
            .insert(MachineId(id.clone()));

        Ok(KvmMachine {
            id: MachineId(id),
            domain_name,
            spec,
            state: MachineState::Running,
            conn: Arc::clone(&self.conn),
        })
    }
}

/// Helper methods for KvmProvider.
impl KvmProvider {
    async fn create_disk(&self, name: &str, storage: &Storage) -> Result<String, KvmError> {
        // TODO: Actual disk creation..
        Ok(format!("/var/lib/libvirt/images/{}.qcow2", name))
    }

    fn build_domain_xml(
        &self,
        name: &str,
        spec: &MachineSpec,
        disk_path: &str,
    ) -> Result<String, KvmError> {
        Ok(format!("<domain>...</domain>"))
    }
}
