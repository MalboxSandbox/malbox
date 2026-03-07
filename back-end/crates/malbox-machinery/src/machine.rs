//! Machine types and specifications.
//!
//! Machine is a concrete struct that represents
//! a running machine instance. It's provider-agnostic and can be
//! passed between different capabilities.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// A running machine instance.
///
/// This is a concrete type that represents a machine allocated by a provider.
/// It's owned by strategies and passed to capabilities as needed.
#[derive(Debug, Clone)]
pub struct Machine {
    /// Unique identifier for this machine.
    pub id: MachineId,
    /// Specification used to create this machine.
    pub spec: MachineSpec,
    /// Current state of the machine.
    pub state: MachineState,
    /// Network endpoint (if available).
    pub endpoint: Option<MachineEndpoint>,
}

impl Machine {
    /// Create a new machine instance.
    pub fn new(id: MachineId, spec: MachineSpec) -> Self {
        Self {
            id,
            spec,
            state: MachineState::Creating,
            endpoint: None,
        }
    }

    /// Get the machine ID.
    pub fn id(&self) -> &MachineId {
        &self.id
    }

    /// Get the machine specification.
    pub fn spec(&self) -> &MachineSpec {
        &self.spec
    }

    /// Get the current state.
    pub fn state(&self) -> MachineState {
        self.state
    }

    /// Get the network endpoint (if available).
    pub fn endpoint(&self) -> Option<&MachineEndpoint> {
        self.endpoint.as_ref()
    }

    /// Update the machine state.
    pub fn set_state(&mut self, state: MachineState) {
        self.state = state;
    }

    /// Update the network endpoint.
    pub fn set_endpoint(&mut self, endpoint: Option<MachineEndpoint>) {
        self.endpoint = endpoint;
    }
}

/// Runtime endpoint information for provisioning.
#[derive(Debug, Clone)]
pub struct MachineEndpoint {
    /// Network address (IPv4 or IPv6).
    pub address: IpAddr,
    /// Machine identifier.
    pub id: String,
    /// Platform type.
    pub platform: Platform,
}

/// Machine specification - describes the desired machine configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MachineSpec {
    pub name: String,
    pub platform: Platform,
    pub resources: Resources,
    pub storage: Storage,
    pub network: Network,
    /// Optional base image/template identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_image: Option<String>,
    /// Optional provisioning configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioning: Option<Provisioning>,
}

/// Provisioning configuration attached to a machine spec.
///
/// Specifies which registered provisioner to use and its configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Provisioning {
    /// Name of the registered provisioner (e.g., "ansible", "native").
    pub provisioner: String,
    /// Provisioner-specific configuration (passed through as TOML).
    #[serde(default = "default_toml_value")]
    pub config: toml::Value,
}

fn default_toml_value() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}

/// Machine identifier.
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

/// Resource requirements.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Resources {
    pub cpus: u32,
    pub memory_mb: u32,
}

/// Storage configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Storage {
    pub boot_disk_gb: u32,
    pub disk_type: DiskType,
}

/// Network configuration.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Network {
    pub mode: NetworkMode,
    pub ip: Option<IpAddr>,
    pub mac: Option<String>,
}

/// Network mode options.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkMode {
    Nat,
    Isolated { subnet: String },
    Bridged { interface: String },
}

/// Disk type options.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DiskType {
    Raw,
    Qcow2,
    Vmdk,
}

/// Machine state representation.
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

/// Machine platform.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Windows,
    Linux,
}
