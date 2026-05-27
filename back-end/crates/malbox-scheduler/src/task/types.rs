//! Task-related type definitions.

use malbox_database::repositories::tasks::Task;
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

use malbox_database::repositories::machinery::MachinePlatform;

/// Compile-time classification of a task's execution model.
///
/// `Task.platform` is `Option<MachinePlatform>` and `Task.snapshot_id` is
/// `Option<uuid::Uuid>`. Both must be present for a VM-based task. The
/// conversion from `MachinePlatform` to the runtime `malbox_machinery::Platform`
/// happens downstream in the worker's VM setup code.
#[derive(Debug, Clone)]
pub enum TaskKind {
    /// Task runs only host plugins - no VM lifecycle needed.
    HostOnly,
    /// Task requires a VM with guest plugins.
    VmBased {
        platform: MachinePlatform,
        snapshot_id: uuid::Uuid,
    },
}

impl TaskKind {
    pub fn classify(task: &Task) -> Self {
        match (&task.platform, &task.snapshot_id) {
            (Some(platform), Some(snapshot_id)) => TaskKind::VmBased {
                platform: platform.clone(),
                snapshot_id: *snapshot_id,
            },
            _ => TaskKind::HostOnly,
        }
    }
}

#[cfg(test)]
mod task_kind_tests {
    use super::*;
    use malbox_database::repositories::machinery::MachinePlatform;
    use malbox_database::repositories::tasks::TaskState;
    use time::PrimitiveDateTime;

    fn make_task(platform: Option<MachinePlatform>, snapshot_id: Option<uuid::Uuid>) -> Task {
        Task {
            id: None,
            target: "sample.exe".into(),
            plugins: vec![],
            profile: None,
            platform,
            timeout: 300,
            enforce_timeout: None,
            priority: 0,
            machine_id: None,
            machine_memory: None,
            machine_cpus: None,
            created_on: PrimitiveDateTime::MIN,
            started_on: None,
            completed_on: None,
            status: TaskState::Pending,
            sample_id: None,
            owner: None,
            tags: None,
            snapshot_id,
        }
    }

    #[test]
    fn classify_host_only_when_no_platform_no_snapshot() {
        let task = make_task(None, None);
        assert!(matches!(TaskKind::classify(&task), TaskKind::HostOnly));
    }

    #[test]
    fn classify_host_only_when_platform_but_no_snapshot() {
        let task = make_task(Some(MachinePlatform::Windows), None);
        assert!(matches!(TaskKind::classify(&task), TaskKind::HostOnly));
    }

    #[test]
    fn classify_vm_based_when_both_present() {
        let id = uuid::Uuid::new_v4();
        let task = make_task(Some(MachinePlatform::Windows), Some(id));
        match TaskKind::classify(&task) {
            TaskKind::VmBased {
                platform,
                snapshot_id,
            } => {
                assert_eq!(platform, MachinePlatform::Windows);
                assert_eq!(snapshot_id, id);
            }
            _ => panic!("expected VmBased"),
        }
    }

    #[test]
    fn classify_host_only_when_snapshot_but_no_platform() {
        let task = make_task(None, Some(uuid::Uuid::new_v4()));
        assert!(matches!(TaskKind::classify(&task), TaskKind::HostOnly));
    }
}
