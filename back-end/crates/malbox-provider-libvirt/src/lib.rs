use std::collections::HashSet;
use std::sync::{Arc, RwLock};
use tracing::{debug, error, warn};
use virt::{connect::Connect, storage_pool::StoragePool};

mod capabilities;
mod domain_xml;
mod error;
mod snapshot_xml;

pub use error::LibvirtError;

use domain_xml::Domain as XmlDomain;
use malbox_machinery::{CreateMachineParams, MachineId};
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
    #[allow(dead_code)]
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

/// Optional provider-specific overrides from machine config.
/// Fields that are `None` use the provider's built-in defaults.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LibvirtOverrides {
    pub nic_model: Option<String>,
    pub cpu_mode: Option<String>,
    pub network: Option<String>,
}

impl LibvirtOverrides {
    /// Deserialize from an opaque toml::Value, or return defaults if None.
    pub fn from_provider_config(config: Option<&malbox_machinery::provider::TomlValue>) -> Self {
        config
            .and_then(|v| malbox_machinery::provider::config::deserialize(v).ok())
            .unwrap_or_default()
    }
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
    /// Create a qcow2 disk volume for a machine.
    ///
    /// If `base_image` is provided, creates a COW overlay backed by it.
    /// Otherwise creates a blank 64GB qcow2 disk.
    pub(crate) async fn create_disk(
        &self,
        name: &str,
        base_image: Option<&str>,
        capacity_bytes: u64,
    ) -> Result<String, LibvirtError> {
        use virt::storage_vol::StorageVol;

        let vol_name = format!("{}.qcow2", name);

        let vol_xml = if let Some(base_image_path) = base_image {
            // Create qcow2 overlay with backing file
            format!(
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
                vol_name, capacity_bytes, base_image_path
            )
        } else {
            // Create blank disk
            format!(
                r#"<volume>
  <name>{}</name>
  <capacity unit='bytes'>{}</capacity>
  <target>
    <format type='qcow2'/>
  </target>
</volume>"#,
                vol_name, capacity_bytes
            )
        };

        let vol = StorageVol::create_xml(&self.storage_pool, &vol_xml, 0)?;
        let path = vol.get_path()?;
        Ok(path)
    }

    /// Get IP address from a domain via DHCP lease lookup.
    ///
    /// Requires a libvirt-managed DHCP network (e.g. the default NAT network).
    /// Does not need anything installed inside the guest.
    pub(crate) fn get_domain_ip(
        domain: &virt::domain::Domain,
    ) -> Result<std::net::IpAddr, LibvirtError> {
        if let Ok(interfaces) =
            domain.interface_addresses(virt::sys::VIR_DOMAIN_INTERFACE_ADDRESSES_SRC_LEASE, 0)
        {
            for iface in &interfaces {
                for addr in &iface.addrs {
                    if let Ok(ip) = addr.addr.parse::<std::net::IpAddr>()
                        && !ip.is_loopback()
                    {
                        return Ok(ip);
                    }
                }
            }
        }

        Err(LibvirtError::Libvirt(
            "No DHCP lease available yet".to_string(),
        ))
    }

    /// Build domain XML from creation parameters.
    pub(crate) fn build_domain_xml(
        &self,
        name: &str,
        params: &CreateMachineParams,
        disk_path: &str,
    ) -> Result<String, LibvirtError> {
        let domain = XmlDomain::from_params(name.to_string(), params, disk_path.to_string());
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
                warn!(target: "libvirt", "{}", message);
            }
            virt::sys::VIR_ERR_ERROR => {
                error!(target: "libvirt", "{}", message);
            }
            _ => {
                debug!(target: "libvirt", "{}", message);
            }
        }
    }

    unsafe {
        virt::sys::virSetErrorFunc(std::ptr::null_mut(), Some(libvirt_error_handler));
    }
}
