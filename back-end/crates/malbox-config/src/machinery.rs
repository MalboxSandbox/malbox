use serde::{Deserialize, Serialize};

/// Generic machinery configuration.
///
/// This configuration applies to all providers and defines defaults for
/// machine allocation, pooling, and infrastructure management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineryConfig {
    /// Manager type to use for machine allocation
    #[serde(default)]
    pub manager: ManagerType,

    /// Default machine specifications
    #[serde(default)]
    pub defaults: MachineDefaults,

    /// Machine pooling configuration (for snapshot-capable providers)
    #[serde(default)]
    pub pool: PoolConfig,
}

impl Default for MachineryConfig {
    fn default() -> Self {
        Self {
            manager: ManagerType::default(),
            defaults: MachineDefaults::default(),
            pool: PoolConfig::default(),
        }
    }
}

/// Manager type for machine allocation.
///
/// Determines the strategy used to allocate and manage machines.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ManagerType {
    /// Pooled manager - uses pre-allocated machines with snapshot restoration.
    /// Requires provider to support Snapshot capability.
    Pooled,

    /// On-demand manager - allocates and provisions machines as needed.
    /// Works with any provider.
    OnDemand,
}

impl Default for ManagerType {
    fn default() -> Self {
        Self::OnDemand
    }
}

impl std::fmt::Display for ManagerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pooled => write!(f, "pooled"),
            Self::OnDemand => write!(f, "on-demand"),
        }
    }
}

/// Default machine specifications.
///
/// These settings apply to all machines unless overridden by provider-specific config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineDefaults {
    /// Number of virtual CPUs
    #[serde(default = "default_cpus")]
    pub cpus: u32,

    /// Memory in megabytes
    #[serde(default = "default_memory")]
    pub memory: u64,

    /// Video memory in megabytes
    #[serde(default = "default_video_memory")]
    pub video_memory: u64,

    /// Path to a golden qcow2 image used as a read-only backing file.
    /// Each machine gets a copy-on-write overlay; the golden image is never modified.
    /// When None, machines are created with blank disks.
    #[serde(default)]
    pub base_image: Option<String>,
}

impl Default for MachineDefaults {
    fn default() -> Self {
        Self {
            cpus: 2,
            memory: 2048,
            video_memory: 128,
            base_image: None,
        }
    }
}

/// Machine pool configuration.
///
/// Pools maintain pre-allocated machines for fast allocation.
/// Only used with snapshot-capable providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    /// Minimum number of machines to keep in the pool
    #[serde(default = "default_min_size")]
    pub min_size: usize,

    /// Maximum number of machines allowed in the pool
    #[serde(default = "default_max_size")]
    pub max_size: usize,

    /// Name of the clean snapshot to restore between uses
    #[serde(default = "default_snapshot_name")]
    pub clean_snapshot_name: String,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_size: 1,
            max_size: 5,
            clean_snapshot_name: "clean".to_string(),
        }
    }
}

// Default value functions
fn default_cpus() -> u32 {
    2
}

fn default_memory() -> u64 {
    2048
}

fn default_video_memory() -> u64 {
    128
}

fn default_min_size() -> usize {
    1
}

fn default_max_size() -> usize {
    5
}

fn default_snapshot_name() -> String {
    "clean".to_string()
}
