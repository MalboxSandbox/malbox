//! Scoped handle for interacting with a running plugin instance.
//!
//! A [`PluginHandle`] is handed out by the plugin manager when a worker
//! acquires a plugin for task execution. It provides a safe, typed interface
//! for executing tasks and releasing the plugin back to the pool when done.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use malbox_plugin_transport::artifact::ArtifactMeta;
use tokio::sync::{Mutex, oneshot};
use tracing::instrument;

use crate::manager::error::{ManagerError, Result};
use crate::manager::instance::{PluginInstance, PluginLifecycle};
use crate::manager::runtime::PluginRuntime;
use crate::registry::manifest::PluginStateConfig;
use crate::registry::types::{PluginEntry, PluginId};
use crate::transport::daemon::GrpcClient;

/// Format of a plugin output payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OutputFormat {
    /// UTF-8 JSON document.
    Json,
    /// Opaque binary data.
    Bytes,
}

/// A single named result produced by a plugin during task execution.
#[derive(Debug, Clone)]
pub struct PluginOutput {
    /// Name the plugin gave this result (e.g. `report`).
    pub result_name: String,
    /// Raw payload bytes.
    pub data: Vec<u8>,
    /// Encoding of `data`.
    pub format: OutputFormat,
    /// Artifact metadata the plugin attached to this output, if any.
    pub artifact: Option<ArtifactMeta>,
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
    /// Control connection for guest plugins; `None` for host plugins.
    control_client: Option<Arc<Mutex<GrpcClient>>>,
}

impl std::fmt::Debug for PluginHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginHandle")
            .field("plugin_id", &self.plugin_id)
            .field("has_control_client", &self.control_client.is_some())
            .finish_non_exhaustive()
    }
}

impl PluginHandle {
    /// Create a new handle for the given plugin instance.
    pub(crate) fn new(
        plugin_id: PluginId,
        entry: Arc<PluginEntry>,
        instance: Arc<Mutex<PluginInstance>>,
        control_client: Option<Arc<Mutex<GrpcClient>>>,
    ) -> Self {
        Self {
            plugin_id,
            entry,
            instance,
            control_client,
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

    pub(crate) fn instance(&self) -> &Arc<Mutex<PluginInstance>> {
        &self.instance
    }

    /// Execute a task on the plugin and collect the resulting outputs.
    ///
    /// `timeout` bounds the whole execution. For guest plugins, `ready` fires
    /// on the plugin's first READY message; host runtimes drop it.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerError::Timeout`] if `timeout` elapses,
    /// [`ManagerError::Crashed`] if the plugin disconnects mid-task,
    /// [`ManagerError::Reported`] if the plugin reports a task error, and
    /// [`ManagerError::ExecutionFailed`], [`ManagerError::Transport`] or
    /// [`ManagerError::Reactor`] for dispatch or transport failures.
    #[instrument(skip_all, fields(plugin = %self.plugin_id(), task_id), err)]
    pub async fn execute_task(
        &self,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
        timeout: Duration,
        ready: Option<oneshot::Sender<()>>,
    ) -> Result<Vec<PluginOutput>> {
        let mut instance = self.instance.lock().await;
        instance
            .runtime
            .execute_task(
                &self.plugin_id,
                &self.entry,
                task_id,
                sample_path,
                config,
                timeout,
                ready,
            )
            .await
    }

    /// Ask a guest plugin to stop observing `task_id` early. Host plugins are
    /// cancelled by killing their process, so this is a no-op for them.
    ///
    /// # Errors
    ///
    /// Returns [`ManagerError::Transport`] if the `StopTask` RPC fails.
    pub async fn stop_task(&self, task_id: i32) -> Result<()> {
        let Some(client) = &self.control_client else {
            return Ok(());
        };
        client
            .lock()
            .await
            .stop_task(task_id)
            .await
            .map_err(ManagerError::Transport)?;
        Ok(())
    }

    /// Release the plugin instance, transitioning it to the appropriate
    /// lifecycle state based on the plugin's [`PluginStateConfig`].
    ///
    /// - **Persistent** plugins return to [`PluginLifecycle::Ready`] so they
    ///   can accept new tasks immediately.
    /// - **Ephemeral** plugins are stopped and their process is killed.
    /// - **Scoped** plugins are returned to [`PluginLifecycle::Ready`].
    pub async fn release(self) {
        let mut instance = self.instance.lock().await;

        match self.entry.manifest.runtime.state {
            PluginStateConfig::Persistent | PluginStateConfig::Scoped => {
                instance.lifecycle = PluginLifecycle::Ready;
            }
            PluginStateConfig::Ephemeral => {
                instance.lifecycle = PluginLifecycle::Stopping;

                if let PluginRuntime::Host(ref mut host) = instance.runtime {
                    let _ = host.process.kill().await;
                }

                instance.lifecycle = PluginLifecycle::Stopped;
            }
        }
    }
}
