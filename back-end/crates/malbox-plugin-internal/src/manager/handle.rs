//! Scoped handle for interacting with a running plugin instance.
//!
//! A [`PluginHandle`] is handed out by the plugin manager when a worker
//! acquires a plugin for task execution. It provides a safe, typed interface
//! for executing tasks and releasing the plugin back to the pool when done.

use std::collections::HashMap;
use std::sync::Arc;

use prost::Message;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

use crate::manager::error::{ManagerError, Result};
use crate::manager::instance::{PluginInstance, PluginLifecycle};
use crate::registry::manifest::{PluginStateConfig, PluginTypeConfig};
use crate::registry::types::{PluginEntry, PluginId};
use crate::transport::grpc::proto;

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
    /// This dispatches the task to the plugin process via the appropriate
    /// transport (IPC for host plugins, gRPC for guest plugins) and waits
    /// for the plugin to produce its result outputs.
    #[instrument(skip_all, fields(plugin = %self.plugin_id(), task_id), err)]
    pub async fn execute_task(
        &self,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<PluginOutput>> {
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
    ) -> Result<Vec<PluginOutput>> {
        // TODO: Implement host IPC task execution — emit TaskStarting event
        // over iceoryx2, wait for result events, and collect into Vec<PluginOutput>.
        warn!(plugin = %self.plugin_id, "host IPC task execution not yet implemented");
        Ok(vec![])
    }

    /// Execute a task on a guest plugin via gRPC streaming.
    async fn execute_guest_task(
        &self,
        task_id: i32,
        sample_path: &str,
        config: HashMap<String, String>,
    ) -> Result<Vec<PluginOutput>> {
        use crate::transport::grpc::proto::{ResultFormat as ProtoFormat, ResultKind};

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

        let mut inline_outputs: Vec<PluginOutput> = Vec::new();
        let mut pending_refs: Vec<proto::ResultRef> = Vec::new();

        // --- Phase 1: drain the control stream ---
        loop {
            match stream.message().await {
                Ok(Some(result)) => {
                    let kind = ResultKind::try_from(result.kind).unwrap_or(ResultKind::Result);
                    let is_final = result.is_final;

                    match kind {
                        ResultKind::Progress => {
                            debug!(
                                plugin = %self.plugin_id,
                                task_id = result.task_id,
                                "received progress update"
                            );
                        }
                        ResultKind::Ready => {
                            debug!(
                                plugin = %self.plugin_id,
                                task_id = result.task_id,
                                "plugin signaled ready"
                            );
                        }
                        ResultKind::Result => {
                            info!(
                                plugin = %self.plugin_id,
                                task_id = result.task_id,
                                result_name = %result.result_name,
                                data_len = result.data.len(),
                                is_final,
                                "received inline task result from guest plugin"
                            );

                            let format = match ProtoFormat::try_from(result.format) {
                                Ok(ProtoFormat::Json) => OutputFormat::Json,
                                _ => OutputFormat::Bytes,
                            };

                            if !result.result_name.is_empty() {
                                inline_outputs.push(PluginOutput {
                                    result_name: result.result_name,
                                    data: result.data,
                                    format,
                                });
                            }
                        }
                        ResultKind::ResultRef => {
                            let ref_msg = proto::ResultRef::decode(result.data.as_slice())
                                .map_err(|e| {
                                    ManagerError::ExecutionFailed(
                                        self.plugin_id.clone(),
                                        format!("failed to decode ResultRef: {e}"),
                                    )
                                })?;
                            info!(
                                plugin = %self.plugin_id,
                                task_id = result.task_id,
                                handle = %ref_msg.handle,
                                result_name = %ref_msg.result_name,
                                size_bytes = ref_msg.size_bytes,
                                "received result ref (will pull after stream completes)"
                            );
                            pending_refs.push(ref_msg);
                        }
                    }

                    if is_final {
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

        // --- Phase 2: pull each ref sequentially on its own stream ---
        let mut outputs = inline_outputs;
        for ref_msg in pending_refs {
            let pulled = pull_result_chunks(client, &self.plugin_id, &ref_msg).await?;
            outputs.push(pulled);
        }

        Ok(outputs)
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

async fn pull_result_chunks(
    client: &mut crate::transport::daemon::GrpcClient,
    plugin_id: &PluginId,
    ref_msg: &proto::ResultRef,
) -> Result<PluginOutput> {
    use crate::transport::grpc::proto::ResultFormat as ProtoFormat;

    let mut stream = client
        .pull_result(ref_msg.handle.clone())
        .await
        .map_err(|e| {
            ManagerError::ExecutionFailed(
                plugin_id.clone(),
                format!(
                    "pull_result RPC for '{}' (handle {}) failed: {}",
                    ref_msg.result_name, ref_msg.handle, e
                ),
            )
        })?;

    let mut buf: Vec<u8> = Vec::with_capacity(ref_msg.size_bytes as usize);
    loop {
        match stream.message().await {
            Ok(Some(chunk)) => {
                buf.extend_from_slice(&chunk.data);
                if chunk.is_last {
                    break;
                }
            }
            Ok(None) => break,
            Err(status) => {
                return Err(ManagerError::ExecutionFailed(
                    plugin_id.clone(),
                    format!(
                        "pull_result stream error for '{}' (handle {}): {}",
                        ref_msg.result_name, ref_msg.handle, status
                    ),
                ));
            }
        }
    }

    let format = match ProtoFormat::try_from(ref_msg.format) {
        Ok(ProtoFormat::Json) => OutputFormat::Json,
        _ => OutputFormat::Bytes,
    };

    info!(
        plugin = %plugin_id,
        result_name = %ref_msg.result_name,
        bytes = buf.len(),
        "pulled large result from guest plugin"
    );

    Ok(PluginOutput {
        result_name: ref_msg.result_name.clone(),
        data: buf,
        format,
    })
}
