use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{net::IpAddr, time::Duration};

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
}

#[derive(Serialize, Deserialize)]
pub struct MachineSpec {
    pub name: String,
    pub platform: Platform,
    pub resources: Resources,
    pub storage: Storage,
    pub network: Network,
}

// For now let's just keep MachineId as a String..
// TODO make this type-safe and maybe use UUID? Or similar.
#[derive(Eq, PartialEq, Hash)]
pub struct MachineId(pub String);

#[derive(Serialize, Deserialize)]
pub struct Resources {
    pub cpus: u32,
    pub memory_mb: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Storage {
    pub boot_disk_gb: u32,
    pub disk_type: DiskType,
}

#[derive(Serialize, Deserialize)]
pub struct Network {
    pub mode: NetworkMode,
    pub ip: Option<IpAddr>,
    pub mac: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub enum NetworkMode {
    Nat,
    Isolated { subnet: String },
    Bridged { interface: String },
}

#[derive(Serialize, Deserialize)]
pub enum DiskType {
    Raw,
    Qcow2,
    Vmdk,
}

/// Machine state representation.
#[derive(Clone)]
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
#[derive(Serialize, Deserialize)]
pub enum Platform {
    Windows,
    Linux,
}
