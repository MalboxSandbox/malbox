use serde::{Deserialize, Serialize};

/// Generic machinery configuration.
///
/// This configuration applies to all providers and defines defaults for
/// machine allocation and infrastructure management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineryConfig {
    /// Default machine specifications
    #[serde(default)]
    pub defaults: MachineDefaults,

    /// Name of the clean snapshot to restore between uses
    #[serde(default = "default_clean_snapshot_name")]
    pub clean_snapshot_name: String,
}

impl Default for MachineryConfig {
    fn default() -> Self {
        Self {
            defaults: MachineDefaults::default(),
            clean_snapshot_name: default_clean_snapshot_name(),
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

    /// Name of a registered image from the image registry.
    /// Resolved to a file path at startup via the database.
    /// When None, machines are created with blank disks.
    #[serde(default)]
    pub image: Option<String>,
}

impl Default for MachineDefaults {
    fn default() -> Self {
        Self {
            cpus: 2,
            memory: 2048,
            video_memory: 128,
            image: None,
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

fn default_clean_snapshot_name() -> String {
    "clean".to_string()
}
