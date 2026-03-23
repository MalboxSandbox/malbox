//! Machine types — runtime representation of a provider-managed machine.
//!
//! The provider-level `Machine` is a lightweight runtime handle: identity,
//! state, and network endpoint. All persistent configuration (platform, arch,
//! image, resources) lives in the database — the provider only needs enough
//! information to manage the VM's lifecycle.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// A machine instance managed by a provider.
///
/// This is the provider's runtime view of a machine. It carries just enough
/// state for lifecycle operations (start, stop, snapshot) and network access.
/// Persistent data (platform, arch, resources, image) lives in the database.
#[derive(Debug, Clone)]
pub struct Machine {
    /// Provider-assigned identifier (e.g., the libvirt domain name suffix).
    pub id: MachineId,
    /// Current power state.
    pub state: MachineState,
    /// Network endpoint, available once the machine has booted and obtained an IP.
    pub endpoint: Option<MachineEndpoint>,
}

impl Machine {
    /// Create a new machine handle in the `Creating` state with no endpoint.
    pub fn new(id: MachineId) -> Self {
        Self {
            id,
            state: MachineState::Creating,
            endpoint: None,
        }
    }

    pub fn id(&self) -> &MachineId {
        &self.id
    }

    pub fn state(&self) -> MachineState {
        self.state
    }

    pub fn endpoint(&self) -> Option<&MachineEndpoint> {
        self.endpoint.as_ref()
    }

    pub fn set_state(&mut self, state: MachineState) {
        self.state = state;
    }

    pub fn set_endpoint(&mut self, endpoint: Option<MachineEndpoint>) {
        self.endpoint = endpoint;
    }
}

/// Network endpoint for connecting to a machine's management interface.
#[derive(Debug, Clone)]
pub struct MachineEndpoint {
    /// IP address (v4 or v6) of the machine.
    pub address: IpAddr,
    /// Provider-side identifier (same as `MachineId.0`).
    pub id: String,
    /// OS platform — used to select connection method (WinRM vs SSH).
    pub platform: Platform,
}

/// Provider-assigned machine identifier.
///
/// For libvirt this is the name suffix: domain `malbox-win10-test` → id `win10-test`.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct MachineId(pub String);

impl std::fmt::Display for MachineId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MachineId {
    fn from(s: String) -> Self {
        MachineId(s)
    }
}

impl From<&str> for MachineId {
    fn from(s: &str) -> Self {
        MachineId(s.to_string())
    }
}

/// Machine power state as reported by the provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachineState {
    Creating,
    Starting,
    Running,
    Stopping,
    Stopped,
    Suspended,
    Failed,
}

/// Target OS platform.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Linux,
}

/// Machine architecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arch {
    X64,
    X86,
}

/// Parameters for creating a new machine via a provider.
///
/// Contains what the provider needs to define a VM. Provider-specific
/// details are passed via `provider_config` and handled internally
/// by each provider implementation.
#[derive(Debug, Clone)]
pub struct CreateMachineParams {
    /// Machine name (provider prefixes this, e.g., `malbox-{name}`).
    pub name: String,
    /// Target OS platform.
    pub platform: Platform,
    /// Target architecture.
    pub arch: Arch,
    /// Number of virtual CPUs.
    pub cpus: u32,
    /// Memory in megabytes.
    pub memory_mb: u32,
    /// Disk size in megabytes.
    pub disk_size_mb: u64,
    /// Path to the base image. `None` for a blank disk.
    pub base_image: Option<String>,
    /// Opaque provider-specific config. Each provider deserializes
    /// the keys it understands, ignores the rest.
    pub provider_config: Option<toml::Value>,
}
