use crate::types::Platform;
use serde::{Deserialize, Serialize};

/// Architecture for a machine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[expect(
    clippy::exhaustive_enums,
    reason = "matched exhaustively by malbox-database architecture mapping"
)]
pub enum Arch {
    /// 64-bit x86 (`x86_64`/AMD64).
    X64,
    /// 32-bit x86.
    X86,
}

/// Declarative machine definition from config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MachineConfig {
    /// Unique machine name used to reference it in tasks and the database.
    pub name: String,
    /// Image the machine is created from.
    pub image: String,
    /// Guest operating system family.
    pub platform: Platform,
    /// Guest CPU architecture.
    pub arch: Arch,
    /// Number of virtual CPUs.
    pub cpus: u32,
    /// Memory in megabytes.
    pub memory: u32,
    /// Disk size in megabytes.
    pub disk_size: u64,
    #[serde(default)]
    /// Provider-specific settings passed through to the provider.
    pub provider_config: Option<toml::Value>,
    #[serde(default)]
    /// Free-form labels used to select machines for tasks.
    pub tags: Vec<String>,
}

/// Validate a list of machine configs.
///
/// Checks: unique names, cpus >= 1, memory > 0, `disk_size` > 0.
///
/// # Errors
///
/// Returns a message naming the first machine that has a duplicate name, zero cpus,
/// zero memory, or zero disk size.
pub fn validate_machine_configs(machines: &[MachineConfig]) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    for m in machines {
        if !seen.insert(&m.name) {
            return Err(format!("Duplicate machine name: '{}'", m.name));
        }
        if m.cpus < 1 {
            return Err(format!("Machine '{}': cpus must be >= 1", m.name));
        }
        if m.memory == 0 {
            return Err(format!("Machine '{}': memory must be > 0", m.name));
        }
        if m.disk_size == 0 {
            return Err(format!("Machine '{}': disk_size must be > 0", m.name));
        }
    }
    Ok(())
}
