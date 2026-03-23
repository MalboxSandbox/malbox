//! Scoped handle for interacting with a running plugin instance.
//!
//! A [`PluginHandle`] is handed out by the plugin manager when a worker
//! acquires a plugin for task execution. It provides a safe, typed interface
//! for executing tasks and releasing the plugin back to the pool when done.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::manager::error::{ManagerError, Result};
use crate::manager::instance::{PluginInstance, PluginLifecycle};
use crate::registry::manifest::{PluginStateConfig, PluginTypeConfig};
use crate::registry::types::{PluginEntry, PluginId};
use crate::transport::messages::events::Payload;

/// A handle to a running plugin instance, used by workers to execute tasks.
///
/// The handle holds a reference-counted pointer to the underlying
/// [`PluginInstance`] behind a [`tokio::sync::Mutex`], ensuring safe
/// concurrent access to the instance state. When the worker is done with the
/// plugin, it must call [`release`](Self::release) to transition the instance
/// back to an appropriate lifecycle state based on its [`PluginStateConfig`].
pub struct PluginHandle {
    plugin_id: PluginId,
    entry: Arc<PluginEntry>,
    instance: Arc<Mutex<PluginInstance>>,
}

impl PluginHandle {
    /// Create a new handle for the given plugin instance.
    pub(crate) fn new(
        plugin_id: PluginId,
        entry: Arc<PluginEntry>,
        instance: Arc<Mutex<PluginInstance>>,
    ) -> Self {
        Self {
            plugin_id,
            entry,
            instance,
        }
    }

    /// Returns a reference to the plugin's unique identifier.
    pub fn plugin_id(&self) -> &PluginId {
        &self.plugin_id
    }

    /// Returns a reference to the plugin's registry entry.
    pub fn entry(&self) -> &PluginEntry {
        &self.entry
    }

    /// Execute a task on the plugin and collect the resulting payloads.
    ///
    /// This dispatches the task to the plugin process via the appropriate
    /// transport (IPC for host plugins, gRPC for guest plugins) and waits
    /// for the plugin to produce its result payloads.
    pub async fn execute_task(
        &self,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<Payload>> {
        let plugin_type = self.entry.manifest.plugin.plugin_type;

        match plugin_type {
            PluginTypeConfig::Host => self.execute_host_task(task_id, sample_path, config).await,
            PluginTypeConfig::Guest => self.execute_guest_task(task_id, sample_path, config).await,
        }
    }

    /// Execute a task on a host plugin via IPC.
    async fn execute_host_task(
        &self,
        _task_id: i32,
        _sample_path: &str,
        _config: HashMap<String, String>,
    ) -> Result<Vec<Payload>> {
        // TODO: Implement host IPC task execution — emit TaskStarting event
        // over iceoryx2, wait for result events, and collect into Vec<Payload>.
        warn!(plugin = %self.plugin_id, "host IPC task execution not yet implemented");
        Ok(vec![])
    }

    /// Execute a task on a guest plugin via gRPC streaming.
    async fn execute_guest_task(
        &self,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<Payload>> {
        use crate::transport::messages::events::TaskEventPayload;

        let mut instance = self.instance.lock().await;
        let client = instance.grpc_client.as_mut().ok_or_else(|| {
            ManagerError::ExecutionFailed(
                self.plugin_id.clone(),
                "guest plugin has no gRPC client".into(),
            )
        })?;

        debug!(plugin = %self.plugin_id, task_id, "executing task on guest plugin via gRPC");

        let mut stream = client
            .execute_task(task_id, sample_path.to_string(), config)
            .await
            .map_err(|e| ManagerError::ExecutionFailed(self.plugin_id.clone(), e.to_string()))?;

        let mut payloads = Vec::new();

        loop {
            match stream.message().await {
                Ok(Some(result)) => {
                    info!(
                        plugin = %self.plugin_id,
                        task_id = result.task_id,
                        result_name = %result.result_name,
                        data_len = result.data.len(),
                        is_final = result.is_final,
                        "received task result from guest plugin"
                    );

                    payloads.push(Payload::Task(TaskEventPayload {
                        task_id: result.task_id,
                    }));

                    if result.is_final {
                        break;
                    }
                }
                Ok(None) => {
                    debug!(plugin = %self.plugin_id, "guest plugin task stream ended");
                    break;
                }
                Err(status) => {
                    warn!(
                        plugin = %self.plugin_id,
                        error = %status,
                        "gRPC stream error during task execution"
                    );
                    return Err(ManagerError::ExecutionFailed(
                        self.plugin_id.clone(),
                        format!("gRPC stream error: {}", status),
                    ));
                }
            }
        }

        Ok(payloads)
    }

    /// Release the plugin instance, transitioning it to the appropriate
    /// lifecycle state based on the plugin's [`PluginStateConfig`].
    ///
    /// - **Persistent** plugins return to [`PluginLifecycle::Ready`] so they
    ///   can accept new tasks immediately.
    /// - **Ephemeral** plugins are stopped and their process is killed.
    /// - **Scoped** plugins are returned to [`PluginLifecycle::Ready`] (full
    ///   scoped release logic is not yet implemented).
    pub async fn release(self) {
        let mut instance = self.instance.lock().await;

        match self.entry.manifest.plugin.state {
            PluginStateConfig::Persistent => {
                instance.lifecycle = PluginLifecycle::Ready;
            }
            PluginStateConfig::Ephemeral => {
                instance.lifecycle = PluginLifecycle::Stopping;

                if let Some(ref mut process) = instance.process {
                    let _ = process.kill().await;
                }

                instance.lifecycle = PluginLifecycle::Stopped;
            }
            PluginStateConfig::Scoped => {
                // TODO: Implement scoped plugin release logic
                instance.lifecycle = PluginLifecycle::Ready;
            }
        }
    }
}
