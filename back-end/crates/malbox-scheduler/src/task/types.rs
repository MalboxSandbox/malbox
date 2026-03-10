//! Task-related type definitions.

use malbox_config::MachineryConfig;
use malbox_database::repositories::machinery::MachinePlatform;
use malbox_database::repositories::tasks::Task;
use malbox_resources::{DiskType, MachineSpec, Network, NetworkMode, Platform, Resources, Storage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub task_id: Option<i32>,
    pub plugin_results: HashMap<String, PluginResult>,
    pub success: bool,
}

impl TaskResult {
    pub fn new(task_id: Option<i32>) -> Self {
        Self {
            task_id,
            plugin_results: HashMap::new(),
            success: true,
        }
    }

    pub fn add_plugin_result(&mut self, plugin_id: String, result: PluginResult) {
        if result.status == PluginStatus::Failed {
            self.success = false;
        }
        self.plugin_results.insert(plugin_id, result);
    }

    pub fn has_failures(&self) -> bool {
        !self.success
    }
}

/// Result of plugin execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResult {
    pub status: PluginStatus,
    pub output: Option<String>,
    pub error: Option<String>,
}

/// Plugin execution status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginStatus {
    Success,
    Failed,
    Skipped,
}

/// Plugin execution context.
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub task: Task,
    pub sandbox: Option<String>, // TODO: Replace with proper sandbox type
    pub resources: ResourceAllocation,
}

/// Resource allocation for task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub cpu_cores: u32,
    pub memory_mb: u32,
    pub storage_gb: u32,
    pub network_allowed: bool,
}

impl Default for ResourceAllocation {
    fn default() -> Self {
        Self {
            cpu_cores: 1,
            memory_mb: 512,
            storage_gb: 10,
            network_allowed: true,
        }
    }
}

/// Build a MachineSpec from a Task using config defaults as fallback.
pub fn task_to_machine_spec(task: &Task, config: &MachineryConfig) -> MachineSpec {
    let platform = match task.platform {
        MachinePlatform::Windows => Platform::Windows,
        MachinePlatform::Linux => Platform::Linux,
    };

    MachineSpec {
        name: format!("task-{}", task.id.unwrap_or(0)),
        platform,
        resources: Resources {
            cpus: task.machine_cpus.unwrap_or(config.defaults.cpus as i32) as u32,
            memory_mb: task.machine_memory.unwrap_or(config.defaults.memory as i64) as u32,
        },
        storage: Storage {
            boot_disk_gb: 64,
            disk_type: DiskType::Qcow2,
        },
        network: Network {
            mode: NetworkMode::Nat,
            ip: None,
            mac: None,
        },
        base_image: config.defaults.image.clone(),
        provisioning: None,
    }
}
