use crate::types::Platform;
use serde::{Deserialize, Serialize};

/// Architecture for a machine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    X64,
    X86,
}

/// Declarative machine definition from config.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MachineConfig {
    pub name: String,
    pub image: String,
    pub platform: Platform,
    pub arch: Arch,
    pub cpus: u32,
    /// Memory in megabytes.
    pub memory: u32,
    /// Disk size in megabytes.
    pub disk_size: u64,
    #[serde(default)]
    pub provider_config: Option<toml::Value>,
}

/// Validate a list of machine configs.
///
/// Checks: unique names, cpus >= 1, memory > 0, disk_size > 0.
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
