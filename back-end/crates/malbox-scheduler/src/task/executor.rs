use super::{
    PluginContext, PluginResult, PluginStatus, ResourceAllocation, TaskResult, store::TaskStore,
};
use crate::error::{Result, TaskError};
use crate::resource::{ResourceError, ResourceManager};
use crate::worker::pool::WorkerPool;
use malbox_database::repositories::tasks::{Task, TaskState};
use malbox_plugin_internal::PluginRegistry;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{debug, error, info, warn};

/// The TaskExecutor manages the actual execution of tasks and their resources.
pub struct TaskExecutor {
    store: Arc<TaskStore>,
    plugin_registry: Arc<PluginRegistry>,
    resource_manager: Arc<ResourceManager>,
}

impl TaskExecutor {
    pub fn new(
        store: Arc<TaskStore>,
        plugin_registry: Arc<PluginRegistry>,
        resource_manager: Arc<ResourceManager>,
    ) -> Self {
        Self {
            store,
            plugin_registry,
            resource_manager,
        }
    }

    pub async fn execute(&self, task: Task, resources: ResourceAllocation) -> Result<TaskResult> {
        // Prepare execution environment
        // let sandbox = self.machinery.create_sandbox(&resources).await?;

        // Update task status
        self.store
            .update_task_state(task.id.expect("Task ID required"), TaskState::Running)
            .await?;

        // Execute plugins in order
        let mut result = TaskResult::new(task.id.clone());

        // TODO: Get plugins from registry based on task
        let plugins: Vec<String> = vec![]; // Placeholder for now

        for plugin in plugins {
            let context = PluginContext {
                task: task.clone(),
                sandbox: None, // TODO: implement sandbox
                resources: resources.clone(),
            };

            // TODO: Implement plugin execution
            let plugin_result = PluginResult {
                status: PluginStatus::Success,
                output: Some("Plugin executed successfully".to_string()),
                error: None,
            };
            result.add_plugin_result("plugin_id".to_string(), plugin_result);

            // Check if we should continue (removed reference to non-existent field)
            // if plugin_result.status == PluginStatus::Failed && task.stop_on_plugin_failure {
            //     break;
            // }
        }

        // Update task status
        let final_status = if result.has_failures() {
            TaskState::Failed
        } else {
            TaskState::Completed
        };

        self.store
            .update_task_state(task.id.expect("Task ID required"), final_status)
            .await?;

        // Release resources
        if let Some(task_id) = task.id {
            // TODO: Implement resource release
            // self.resource_manager.release(&task_id).await?;
        }

        Ok(result)
    }

    pub async fn execute_batch(
        &self,
        tasks: Vec<Task>,
        resources: ResourceAllocation,
    ) -> Result<Vec<Result<TaskResult>>> {
        let mut results = Vec::new();
        for task in tasks {
            let result = self.execute(task, resources.clone()).await;
            results.push(result);
        }
        Ok(results)
    }

    async fn execute_plugin(
        &self,
        plugin_id: &str,
        context: PluginContext,
    ) -> Result<PluginResult> {
        todo!()
    }
}
