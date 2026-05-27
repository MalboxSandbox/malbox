//! Scoped handle for interacting with a running plugin instance.
//!
//! A [`PluginHandle`] is handed out by the plugin manager when a worker
//! acquires a plugin for task execution. It provides a safe, typed interface
//! for executing tasks and releasing the plugin back to the pool when done.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::instrument;

use crate::manager::error::Result;
use crate::manager::instance::{PluginInstance, PluginLifecycle};
use crate::manager::runtime::PluginRuntime;
use crate::registry::manifest::PluginStateConfig;
use crate::registry::types::{PluginEntry, PluginId};

/// Format of a plugin output payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Bytes,
}

/// A single named result produced by a plugin during task execution.
#[derive(Debug, Clone)]
pub struct PluginOutput {
    pub result_name: String,
    pub data: Vec<u8>,
    pub format: OutputFormat,
}

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

    /// Execute a task on the plugin and collect the resulting outputs.
    ///
    /// Delegates to the typed runtime (host IPC or guest gRPC) stored on
    /// the plugin instance.
    #[instrument(skip_all, fields(plugin = %self.plugin_id(), task_id), err)]
    pub async fn execute_task(
        &self,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<PluginOutput>> {
        let mut instance = self.instance.lock().await;
        instance
            .runtime
            .execute_task(&self.plugin_id, &self.entry, task_id, sample_path, config)
            .await
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

        match self.entry.manifest.runtime.state {
            PluginStateConfig::Persistent => {
                instance.lifecycle = PluginLifecycle::Ready;
            }
            PluginStateConfig::Ephemeral => {
                instance.lifecycle = PluginLifecycle::Stopping;

                if let PluginRuntime::Host(ref mut host) = instance.runtime {
                    let _ = host.process.kill().await;
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
