use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{any::Any, net::IpAddr, time::Duration};

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

/// Machine trait that all machine types must implement.
#[async_trait]
pub trait Machine: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    fn id(&self) -> &MachineId;
    fn spec(&self) -> &MachineSpec;
    fn state(&self) -> MachineState;

    async fn start(&mut self) -> Result<(), Self::Error>;
    async fn stop(&mut self) -> Result<(), Self::Error>;
    async fn reboot(&mut self) -> Result<(), Self::Error>;
    async fn wait_ready(&self, timeout: Duration) -> Result<(), Self::Error>;

    /// Wait for machine to have network connectivity.
    async fn wait_network(&self, timeout: Duration) -> Result<(), Self::Error>;

    /// Get network endpoint for provisioning.
    /// Returns None if machine is not in a network-accessible state.
    fn endpoint(&self) -> Option<MachineEndpoint>;

    /// For downcasting support (internal use).
    fn as_any(&self) -> &dyn Any;
    /// For mutable downcasting support (internal use).
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

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

// For now let's just keep MachineId as a String..
// TODO make this type-safe and maybe use UUID? Or similar.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct MachineId(pub String);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Resources {
    pub cpus: u32,
    pub memory_mb: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Storage {
    pub boot_disk_gb: u32,
    pub disk_type: DiskType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Network {
    pub mode: NetworkMode,
    pub ip: Option<IpAddr>,
    pub mac: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum NetworkMode {
    Nat,
    Isolated { subnet: String },
    Bridged { interface: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DiskType {
    Raw,
    Qcow2,
    Vmdk,
}

/// Machine state representation.
#[derive(Debug, Clone, Copy)]
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
// TODO: shared enum across crates.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Platform {
    Windows,
    Linux,
}

/// Provisioning configuration
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum Provisioning {
    Ansible { playbook: String },
}

/// Object-safe machine trait for dynamic dispatch.
///
/// This trait provides a type-erased interface to machines, allowing them to be
/// stored and used without knowing their concrete types at compile time.
#[async_trait]
pub trait DynMachine: Send + Sync {
    fn id(&self) -> &MachineId;
    fn spec(&self) -> &MachineSpec;
    fn state(&self) -> MachineState;
    fn endpoint(&self) -> Option<MachineEndpoint>;

    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn reboot(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn wait_ready(&self, timeout: Duration) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    async fn wait_network(&self, timeout: Duration) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// For downcasting support (internal use).
    fn as_any(&self) -> &dyn Any;
    /// For mutable downcasting support (internal use).
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Blanket implementation of DynMachine for all Machine types.
#[async_trait]
impl<M: Machine + 'static> DynMachine for M {
    fn id(&self) -> &MachineId {
        Machine::id(self)
    }

    fn spec(&self) -> &MachineSpec {
        Machine::spec(self)
    }

    fn state(&self) -> MachineState {
        Machine::state(self)
    }

    fn endpoint(&self) -> Option<MachineEndpoint> {
        Machine::endpoint(self)
    }

    async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Machine::start(self).await.map_err(|e| Box::new(e) as _)
    }

    async fn stop(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Machine::stop(self).await.map_err(|e| Box::new(e) as _)
    }

    async fn reboot(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Machine::reboot(self).await.map_err(|e| Box::new(e) as _)
    }

    async fn wait_ready(&self, timeout: Duration) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Machine::wait_ready(self, timeout).await.map_err(|e| Box::new(e) as _)
    }

    async fn wait_network(&self, timeout: Duration) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Machine::wait_network(self, timeout).await.map_err(|e| Box::new(e) as _)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
