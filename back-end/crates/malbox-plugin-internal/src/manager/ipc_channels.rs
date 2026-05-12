//! Daemon-side IPC task channels for a single host plugin.
//!
//! Uses request/response pattern for task dispatch with streaming responses.

use malbox_plugin_transport::ipc::headers::{
    FLAG_HAS_MORE_CHUNKS, FLAG_IS_FINAL, ResponseKind, ResultFormat, TaskRequestHeader,
};
use malbox_plugin_transport::ipc::notify::NotifyKind;
use malbox_plugin_transport::ipc::{IpcService, Node};
use malbox_plugin_transport::ipc::{
    PluginEventSubscriber, PluginNotifier, ResultSubscriber, TaskClient, TaskPendingResponse,
};

use crate::manager::error::{ManagerError, Result};
use crate::manager::handle::{OutputFormat, PluginOutput};
use crate::registry::types::PluginId;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;
use tracing::{debug, warn};

/// Postcard-serialized task request payload (daemon side).
#[derive(Serialize)]
struct TaskRequestPayload<'a> {
    pub sample_path: &'a str,
    pub config: Vec<(&'a str, &'a str)>,
}

/// Daemon-side channels for task execution with a host plugin.
pub struct HostTaskChannels {
    pub task_client: TaskClient,
    pub plugin_notifier: PluginNotifier,
    pub event_subscriber: PluginEventSubscriber,
    pub result_subscriber: ResultSubscriber,
}

impl HostTaskChannels {
    pub fn new(node: &Node<IpcService>, plugin_id: &str) -> Result<Self> {
        let task_client = TaskClient::new(node, plugin_id).map_err(ManagerError::Transport)?;

        let plugin_notifier =
            PluginNotifier::new(node, plugin_id).map_err(ManagerError::Transport)?;

        let event_subscriber =
            PluginEventSubscriber::new(node, plugin_id).map_err(ManagerError::Transport)?;

        let result_subscriber =
            ResultSubscriber::new(node, plugin_id).map_err(ManagerError::Transport)?;

        Ok(Self {
            task_client,
            plugin_notifier,
            event_subscriber,
            result_subscriber,
        })
    }

    /// Dispatch a task via request/response and collect results.
    pub fn execute_task(
        &self,
        plugin_id: &PluginId,
        task_id: i32,
        sample_path: &str,
        config: &HashMap<String, String>,
        timeout: Duration,
    ) -> Result<Vec<PluginOutput>> {
        let config_vec: Vec<(&str, &str)> = config
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let payload = TaskRequestPayload {
            sample_path,
            config: config_vec,
        };

        let payload_bytes = postcard::to_allocvec(&payload).map_err(|e| {
            ManagerError::ExecutionFailed(
                plugin_id.clone(),
                format!("failed to serialize task request: {e}"),
            )
        })?;

        let header = TaskRequestHeader {
            task_id,
            request_type: 0,
            _reserved: [0; 3],
        };

        let pending = self
            .task_client
            .send(&header, &payload_bytes)
            .map_err(|e| {
                ManagerError::ExecutionFailed(
                    plugin_id.clone(),
                    format!("failed to send task request: {e}"),
                )
            })?;

        // Wake the plugin immediately so it drains the request from shared
        // memory. The plugin's WaitSet also has a 500ms interval as a fallback.
        let _ = self.plugin_notifier.wake(NotifyKind::TaskAvailable);

        self.collect_responses(plugin_id, task_id, pending, timeout)
    }

    fn collect_responses(
        &self,
        plugin_id: &PluginId,
        task_id: i32,
        pending: TaskPendingResponse,
        timeout: Duration,
    ) -> Result<Vec<PluginOutput>> {
        let mut outputs = Vec::new();
        let deadline = Instant::now() + timeout;
        let mut server_connected = false;

        // Accumulator for multi-chunk results.
        let mut chunked_name: Option<String> = None;
        let mut chunked_data: Vec<u8> = Vec::new();
        let mut chunked_format = OutputFormat::Bytes;

        loop {
            if Instant::now() > deadline {
                return Err(ManagerError::ExecutionFailed(
                    plugin_id.clone(),
                    format!("task {task_id} timed out"),
                ));
            }

            match pending.try_recv() {
                Ok(Some(response)) => {
                    server_connected = true;
                    let h = &response.header;

                    if h.flags & FLAG_IS_FINAL != 0 {
                        if let Some(name) = chunked_name.take() {
                            outputs.push(PluginOutput {
                                result_name: name,
                                data: std::mem::take(&mut chunked_data),
                                format: chunked_format,
                            });
                        }
                        break;
                    }

                    let kind = ResponseKind::try_from(h.kind).unwrap_or(ResponseKind::Result);

                    match kind {
                        ResponseKind::Result => {
                            let format = match ResultFormat::try_from(h.format) {
                                Ok(ResultFormat::Json) => OutputFormat::Json,
                                _ => OutputFormat::Bytes,
                            };

                            let has_more = h.flags & FLAG_HAS_MORE_CHUNKS != 0;

                            let name_len = h.name_len as usize;
                            let (result_name, data) = if name_len > 0
                                && response.payload.len() >= name_len
                            {
                                let name = String::from_utf8_lossy(&response.payload[..name_len])
                                    .into_owned();
                                let data = response.payload[name_len..].to_vec();
                                (name, data)
                            } else {
                                (String::new(), response.payload)
                            };

                            if has_more {
                                if h.chunk_index == 0 {
                                    if let Some(prev_name) = chunked_name.take() {
                                        outputs.push(PluginOutput {
                                            result_name: prev_name,
                                            data: std::mem::take(&mut chunked_data),
                                            format: chunked_format,
                                        });
                                    }
                                    chunked_name = Some(result_name);
                                    chunked_data = data;
                                    chunked_format = format;
                                    if h.total_size > 0 {
                                        chunked_data.reserve(h.total_size as usize);
                                    }
                                } else {
                                    chunked_data.extend_from_slice(&data);
                                }
                            } else if chunked_name.is_some() {
                                chunked_data.extend_from_slice(&data);
                                outputs.push(PluginOutput {
                                    result_name: chunked_name.take().unwrap(),
                                    data: std::mem::take(&mut chunked_data),
                                    format: chunked_format,
                                });
                            } else {
                                debug!(
                                    plugin = %plugin_id,
                                    task_id,
                                    %result_name,
                                    data_len = data.len(),
                                    "received result"
                                );
                                outputs.push(PluginOutput {
                                    result_name,
                                    data,
                                    format,
                                });
                            }
                        }
                        ResponseKind::FileRef => {
                            let path = String::from_utf8_lossy(&response.payload).to_string();
                            match std::fs::read(&path) {
                                Ok(data) => {
                                    let format = match ResultFormat::try_from(h.format) {
                                        Ok(ResultFormat::Json) => OutputFormat::Json,
                                        _ => OutputFormat::Bytes,
                                    };
                                    outputs.push(PluginOutput {
                                        result_name: path
                                            .rsplit('/')
                                            .next()
                                            .unwrap_or(&path)
                                            .to_string(),
                                        data,
                                        format,
                                    });
                                }
                                Err(e) => {
                                    warn!(
                                        plugin = %plugin_id,
                                        task_id,
                                        path = %path,
                                        error = %e,
                                        "failed to read file ref"
                                    );
                                }
                            }
                        }
                        ResponseKind::Progress => {
                            debug!(plugin = %plugin_id, task_id, "progress update");
                        }
                        ResponseKind::Ready => {
                            debug!(plugin = %plugin_id, task_id, "plugin ready");
                        }
                        ResponseKind::Error => {
                            let msg = String::from_utf8_lossy(&response.payload);
                            return Err(ManagerError::ExecutionFailed(
                                plugin_id.clone(),
                                format!("plugin task error: {msg}"),
                            ));
                        }
                    }
                }
                Ok(None) => {
                    if pending.is_connected() {
                        server_connected = true;
                    } else if server_connected {
                        if let Some(name) = chunked_name.take() {
                            outputs.push(PluginOutput {
                                result_name: name,
                                data: std::mem::take(&mut chunked_data),
                                format: chunked_format,
                            });
                        }
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(e) => {
                    return Err(ManagerError::ExecutionFailed(
                        plugin_id.clone(),
                        format!("response receive error: {e}"),
                    ));
                }
            }
        }

        Ok(outputs)
    }
}
