use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use virt::{connect::Connect, storage_pool::StoragePool};

mod capabilities;
mod domain_xml;
mod error;
mod snapshot_xml;

pub use error::LibvirtError;

use domain_xml::Domain as XmlDomain;
use malbox_machinery::{MachineId, MachineSpec, Storage};
use malbox_machinery_macros::RegisterProvider;

/// Libvirt/KVM provider for managing virtual machines.
///
/// This provider uses libvirt to manage KVM virtual machines. It supports:
/// - Machine allocation and deallocation (via Allocate capability)
/// - VM snapshots (via Snapshot capability)
/// - VM cloning (via Clone capability)
#[derive(RegisterProvider)]
#[provider(name = "libvirt")]
#[capability(Snapshot)]
#[capability(Clone)]
pub struct LibvirtProvider {
    conn: Arc<Connect>,
    config: LibvirtConfig,
    storage_pool: Arc<StoragePool>,
    allocated: Arc<RwLock<HashSet<MachineId>>>,
}

/// Configuration for LibvirtProvider.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LibvirtConfig {
    pub uri: String,
    #[serde(default = "default_storage_pool")]
    pub storage_pool: String,
}

fn default_storage_pool() -> String {
    "default".to_string()
}

impl LibvirtProvider {
    /// Create a new LibvirtProvider from configuration.
    ///
    /// Takes a TOML value and deserializes it to LibvirtConfig.
    pub fn new(config: &malbox_machinery::provider::TomlValue) -> Result<Self, LibvirtError> {
        // Use machinery's config helper to deserialize without depending on toml directly
        let config: LibvirtConfig = malbox_machinery::provider::config::deserialize(config)
            .map_err(|e| LibvirtError::Libvirt(format!("Failed to deserialize config: {}", e)))?;

        Self::with_config(config)
    }

    /// Create a new LibvirtProvider with custom configuration.
    fn with_config(config: LibvirtConfig) -> Result<Self, LibvirtError> {
        // Route libvirt's C-level error output through tracing instead of stderr.
        install_libvirt_error_handler();

        let conn = Arc::new(Connect::open(Some(&config.uri))?);

        let storage_pool = StoragePool::lookup_by_name(&conn, &config.storage_pool)?;

        if !storage_pool.is_active()? {
            // Flags: https://libvirt.org/html/libvirt-libvirt-storage.html#virStoragePoolCreateFlags
            // Flag `0`: Create the pool but do not perform pool build
            storage_pool.create(0)?;
        }

        // Track existing malbox-managed domains
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

    /// Get the libvirt connection.
    pub fn connection(&self) -> &Arc<Connect> {
        &self.conn
    }

    /// Get the storage pool.
    pub fn storage_pool(&self) -> &Arc<StoragePool> {
        &self.storage_pool
    }

    /// Get the allocated machines tracking set.
    pub fn allocated(&self) -> &Arc<RwLock<HashSet<MachineId>>> {
        &self.allocated
    }
}

/// Helper methods for LibvirtProvider.
impl LibvirtProvider {
    /// Create a disk volume for a machine.
    pub(crate) async fn create_disk(
        &self,
        name: &str,
        storage: &Storage,
        base_image: Option<&str>,
    ) -> Result<String, LibvirtError> {
        use malbox_machinery::DiskType;
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

    /// Build domain XML from machine spec.
    pub(crate) fn build_domain_xml(
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

/// Replace libvirt's default stderr error handler with one that logs through tracing.
fn install_libvirt_error_handler() {
    extern "C" fn libvirt_error_handler(_data: *mut std::ffi::c_void, err: virt::sys::virErrorPtr) {
        if err.is_null() {
            return;
        }

        let message = unsafe {
            if (*err).message.is_null() {
                "unknown libvirt error".to_string()
            } else {
                std::ffi::CStr::from_ptr((*err).message)
                    .to_string_lossy()
                    .trim()
                    .to_string()
            }
        };

        let level = unsafe { (*err).level };

        match level as virt::sys::virErrorLevel {
            virt::sys::VIR_ERR_WARNING => {
                tracing::warn!(target: "libvirt", "{}", message);
            }
            virt::sys::VIR_ERR_ERROR => {
                tracing::error!(target: "libvirt", "{}", message);
            }
            _ => {
                tracing::debug!(target: "libvirt", "{}", message);
            }
        }
    }

    unsafe {
        virt::sys::virSetErrorFunc(std::ptr::null_mut(), Some(libvirt_error_handler));
    }
}
