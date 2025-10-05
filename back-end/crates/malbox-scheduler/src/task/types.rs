//! Task-related type definitions.

use malbox_database::repositories::tasks::Task;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

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
