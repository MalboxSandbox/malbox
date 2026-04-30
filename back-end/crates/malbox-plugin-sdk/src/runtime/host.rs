//! Host plugin runtime — IPC-based event loop.
//!
//! Host plugins run on the same machine as the daemon and communicate via
//! iceoryx2 shared-memory IPC. The runtime polls the receiver for incoming
//! events and dispatches them to the plugin.
//!
//! Gated behind the `host` feature flag.

use crate::context::Context;
use crate::error::{Result, SdkError};
use crate::plugin::HostPlugin;
use crate::types::{PluginMeta, Task};

use malbox_plugin_transport::grpc::proto;
use malbox_plugin_transport::ipc::{
    EventEmitter, EventReceiver, IpcService, NodeBuilder, daemon_channel, plugin_channel,
    tasks::{IpcTaskResultHeader, TaskRequestReceiver, TaskResultPublisher},
};
use malbox_plugin_transport::messages::events::Event;
use malbox_plugin_transport::traits::{TransportEmitter, TransportReceiver};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{debug, error, info, instrument};

/// Host plugin runtime.
pub struct HostRuntime<P> {
    plugin: P,
    meta: PluginMeta,
    receiver: EventReceiver,
    emitter: EventEmitter,
    task_request_rx: TaskRequestReceiver,
    task_result_tx: TaskResultPublisher,
    shutdown: AtomicBool,
}

impl<P: HostPlugin> HostRuntime<P> {
    /// Create a new host plugin runtime.
    pub fn new(plugin: P, meta: PluginMeta) -> Result<Self> {
        info!(plugin = %meta.name, "Initializing host runtime");

        let node = NodeBuilder::new()
            .create::<IpcService>()
            .map_err(|e| SdkError::Init(format!("Failed to create IPC node: {}", e)))?;

        let receiver = EventReceiver::new(&node, daemon_channel::EVENTS, daemon_channel::PAYLOADS)
            .map_err(|e| SdkError::Init(format!("Failed to create event receiver: {}", e)))?;

        let emitter = EventEmitter::new(&node, plugin_channel::EVENTS, plugin_channel::PAYLOADS)
            .map_err(|e| SdkError::Init(format!("Failed to create event emitter: {}", e)))?;

        let plugin_id = std::env::var("MALBOX_PLUGIN_ID")
            .map_err(|_| SdkError::Init("MALBOX_PLUGIN_ID environment variable not set".into()))?;

        let task_request_rx = TaskRequestReceiver::new(&node, &plugin_id).map_err(|e| {
            SdkError::Init(format!("Failed to create task request receiver: {}", e))
        })?;

        let task_result_tx = TaskResultPublisher::new(&node, &plugin_id).map_err(|e| {
            SdkError::Init(format!("Failed to create task result publisher: {}", e))
        })?;

        info!(plugin = %meta.name, "Host runtime initialized");

        Ok(Self {
            plugin,
            meta,
            receiver,
            emitter,
            task_request_rx,
            task_result_tx,
            shutdown: AtomicBool::new(false),
        })
    }

    /// Run the plugin event loop. Blocks until shutdown.
    #[instrument(skip_all, fields(plugin = %self.meta.name), err)]
    pub fn run(&self) -> Result<()> {
        let event_ctx = Context::new(&self.emitter, None);

        // Call on_start
        self.plugin.on_start(HashMap::new())?;

        // Emit PluginStarted
        event_ctx.emit_event(Event::PluginStarted { plugin_id: 0 })?;
        info!(plugin = %self.meta.name, "Plugin started, entering event loop");

        // Event loop
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                info!(plugin = %self.meta.name, "Shutdown signal received");
                break;
            }

            match self.receiver.wait(Duration::from_millis(100)) {
                Ok(Some(event)) => {
                    debug!(?event, "Received event");

                    if matches!(event, Event::DaemonShutdown) {
                        self.shutdown.store(true, Ordering::Relaxed);
                    }

                    if let Event::TaskStarting { task_id } = event {
                        match self.task_request_rx.try_recv() {
                            Ok(Some(request_bytes)) => {
                                self.handle_task(task_id, &request_bytes);
                                continue;
                            }
                            Ok(None) => {
                                // Not for us - fall through to on_event
                            }
                            Err(e) => {
                                error!(
                                    plugin = %self.meta.name,
                                    error = %e,
                                    "Failed to read task request channel"
                                );
                            }
                        }
                    }

                    if let Err(e) = self.plugin.on_event(event, &event_ctx) {
                        error!(plugin = %self.meta.name, error = %e, "Handler error");
                    }
                }
                Ok(None) => continue,
                Err(e) => {
                    error!(plugin = %self.meta.name, error = %e, "Transport error");
                }
            }
        }

        // Call on_stop
        if let Err(e) = self.plugin.on_stop() {
            error!(plugin = %self.meta.name, error = %e, "on_stop error");
        }

        // Emit PluginStopped
        let _ = event_ctx.emit_event(Event::PluginStopped { plugin_id: 0 });

        info!(plugin = %self.meta.name, "Plugin runtime exited");
        Ok(())
    }

    fn handle_task(&self, task_id: i32, request_bytes: &[u8]) {
        let request: proto::TaskRequest = match prost::Message::decode(request_bytes) {
            Ok(r) => r,
            Err(e) => {
                error!(task_id, error = %e, "Failed to decode TaskRequest");
                let _ = self.task_result_tx.publish_final(task_id);
                let _ = self.emitter.emit(Event::TaskFailed { task_id });
                return;
            }
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel(64);

        let task = Task::new(task_id, PathBuf::from(&request.sample_path), request.config);

        let task_ctx = Context::new(&self.emitter, Some(tx)).with_task_id(task_id);

        // Use std::thread::scope for safe borrowing of &self references in drain thread
        let task_result = std::thread::scope(|s| {
            let result_tx = &self.task_result_tx;

            s.spawn(move || {
                drain_results(&mut rx, result_tx);
            });

            self.plugin.on_task(task, &task_ctx)
        });

        // Drop context to close the mpsc sender (drain thread will finish)
        drop(task_ctx);

        // Send final marker
        if let Err(e) = self.task_result_tx.publish_final(task_id) {
            error!(task_id, error = %e, "Failed to publish final marker");
        }

        match task_result {
            Ok(()) => {
                let _ = self.emitter.emit(Event::TaskCompleted { task_id });
                info!(plugin = %self.meta.name, task_id, "Task completed");
            }
            Err(e) => {
                error!(plugin = %self.meta.name, task_id, error = %e, "Task failed");
                let _ = self.emitter.emit(Event::TaskFailed { task_id });
            }
        }
    }
}

fn drain_results(
    rx: &mut tokio::sync::mpsc::Receiver<std::result::Result<proto::TaskResult, tonic::Status>>,
    result_tx: &TaskResultPublisher,
) {
    while let Some(msg) = rx.blocking_recv() {
        match msg {
            Ok(task_result) => {
                let task_id = task_result.task_id;
                let format = task_result.format;
                let kind = task_result.kind;

                let payload_bytes = prost::Message::encode_to_vec(&proto::TaskResult {
                    task_id: task_result.task_id,
                    result_name: task_result.result_name,
                    data: task_result.data,
                    format: task_result.format,
                    is_final: false,
                    kind: task_result.kind,
                });

                let header = IpcTaskResultHeader {
                    task_id,
                    format,
                    kind,
                    is_final: 0,
                    _padding: [0; 3],
                    payload_len: payload_bytes.len() as u32,
                };

                if let Err(e) = result_tx.publish(&header, &payload_bytes) {
                    error!("Failed to publish result to IPC: {e}");
                    break;
                }
            }
            Err(status) => {
                error!("Result channel error: {status}");
                break;
            }
        }
    }
}
