//! Host plugin runtime - WaitSet-based event loop over iceoryx2 IPC.
//!
//! The runtime multiplexes several IPC channels into a single event loop:
//!
//! - **Task dispatch** - request/response channel from the daemon.
//! - **Daemon events** - pub/sub for lifecycle and system signals.
//! - **Result chaining** - subscribes to other plugins' result channels
//!   so a plugin can react to upstream outputs.
//! - **WaitSet wakeup** - efficient notification-driven wakeup (no polling).
//!
//! Gated behind the `host` feature flag.

use crate::context::Context;
use crate::error::{Result, SdkError};
use crate::meta::PluginMeta;
use crate::plugin::HostPlugin;

use malbox_plugin_transport::ipc::headers::{
    EventKind, FLAG_IS_FINAL, ResponseKind, ResultHeader, TaskResponseHeader,
};
use malbox_plugin_transport::ipc::{
    ActiveTaskRequest, DaemonEventSubscriber, EventHeader, PluginEventPublisher,
    PluginWakeupListener, ResultPublisher, ResultSubscriber, TaskServer,
};
use malbox_plugin_transport::ipc::{
    CallbackProgression, IpcService, Node, NodeBuilder, WaitSetBuilder,
};
use malbox_plugin_transport::messages::events::Event;
use malbox_plugin_transport::traits::TransportEmitter;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, instrument};

const POLL_INTERVAL_MS: u64 = 500;
const RESULT_CHANNEL_CAPACITY: usize = 256;

/// Task request payload serialized with postcard over IPC.
#[derive(Serialize, Deserialize)]
pub struct TaskRequestPayload<'a> {
    /// Path to the sample file on the host filesystem.
    pub sample_path: &'a str,
    /// Key-value configuration pairs for this task.
    pub config: Vec<(&'a str, &'a str)>,
}

/// Owned version for deserialization.
#[derive(Deserialize)]
struct TaskRequestPayloadOwned {
    pub sample_path: String,
    pub config: Vec<(String, String)>,
}

/// Host plugin runtime with WaitSet-driven event loop.
pub struct HostRuntime<P> {
    plugin: P,
    meta: PluginMeta,
    #[allow(dead_code)]
    plugin_id: String,

    // iceoryx2 node - must outlive all ports created from it
    _node: Node<IpcService>,

    // Task dispatch (request/response)
    task_server: TaskServer,

    // Events
    daemon_event_sub: DaemonEventSubscriber,
    plugin_event_pub: PluginEventPublisher,

    // Results (chaining) - publisher used when result forwarding is wired up
    #[allow(dead_code)]
    result_pub: ResultPublisher,
    result_subscriptions: Vec<(String, ResultSubscriber)>,

    // WaitSet wakeup
    wakeup_listener: PluginWakeupListener,

    // Lifecycle
    shutdown: AtomicBool,
}

impl<P: HostPlugin> HostRuntime<P> {
    /// Create a new host plugin runtime.
    ///
    /// `subscribed_plugins` is the list of plugin IDs this plugin subscribes to
    /// for event/result chaining (declared via macro).
    pub fn new(plugin: P, meta: PluginMeta, subscribed_plugins: &[&str]) -> Result<Self> {
        info!(plugin = %meta.name(), "Initializing host runtime");

        let node = NodeBuilder::new()
            .create::<IpcService>()
            .map_err(|e| SdkError::Init(format!("Failed to create IPC node: {e}")))?;

        let plugin_id = std::env::var("MALBOX_PLUGIN_ID")
            .map_err(|_| SdkError::Init("MALBOX_PLUGIN_ID environment variable not set".into()))?;

        let task_server = TaskServer::new(&node, &plugin_id)
            .map_err(|e| SdkError::Init(format!("Failed to create task server: {e}")))?;

        let daemon_event_sub = DaemonEventSubscriber::new(&node).map_err(|e| {
            SdkError::Init(format!("Failed to create daemon event subscriber: {e}"))
        })?;

        let plugin_event_pub = PluginEventPublisher::new(&node, &plugin_id)
            .map_err(|e| SdkError::Init(format!("Failed to create plugin event publisher: {e}")))?;

        let result_pub = ResultPublisher::new(&node, &plugin_id)
            .map_err(|e| SdkError::Init(format!("Failed to create result publisher: {e}")))?;

        let wakeup_listener = PluginWakeupListener::new(&node, &plugin_id)
            .map_err(|e| SdkError::Init(format!("Failed to create wakeup listener: {e}")))?;

        // Subscribe to other plugins' result channels for chaining
        let mut result_subscriptions = Vec::new();
        for &sub_plugin_id in subscribed_plugins {
            let sub = ResultSubscriber::new(&node, sub_plugin_id).map_err(|e| {
                SdkError::Init(format!(
                    "Failed to subscribe to plugin '{sub_plugin_id}' results: {e}"
                ))
            })?;
            result_subscriptions.push((sub_plugin_id.to_string(), sub));
        }

        info!(plugin = %meta.name(), "Host runtime initialized");

        Ok(Self {
            plugin,
            meta,
            plugin_id,
            _node: node,
            task_server,
            daemon_event_sub,
            plugin_event_pub,
            result_pub,
            result_subscriptions,
            wakeup_listener,
            shutdown: AtomicBool::new(false),
        })
    }

    /// Run the plugin event loop. Blocks until shutdown.
    #[instrument(skip_all, fields(plugin = %self.meta.name()), err)]
    pub fn run(&self) -> Result<()> {
        self.plugin.on_start(HashMap::new())?;

        // Emit PluginStarted
        let header = EventHeader {
            event_type: EventKind::PluginStarted as u16,
            source_plugin_id: 0,
            associated_id: 0,
            _reserved: [0; 6],
        };
        if let Err(e) = self.plugin_event_pub.emit_signal(&header) {
            error!(plugin = %self.meta.name(), error = %e, "Failed to emit PluginStarted event");
        }
        info!(plugin = %self.meta.name(), "Plugin started, entering WaitSet event loop");

        // Build WaitSet
        let waitset = WaitSetBuilder::new()
            .create::<IpcService>()
            .map_err(|e| SdkError::Init(format!("Failed to create WaitSet: {e}")))?;

        let guard = waitset
            .attach_notification(self.wakeup_listener.listener())
            .map_err(|e| SdkError::Init(format!("Failed to attach to WaitSet: {e}")))?;

        // Also attach an interval for periodic checks (shutdown, etc.)
        let _interval_guard = waitset
            .attach_interval(core::time::Duration::from_millis(POLL_INTERVAL_MS))
            .map_err(|e| SdkError::Init(format!("Failed to attach interval: {e}")))?;

        let result = waitset.wait_and_process(|attachment_id| {
            if self.shutdown.load(Ordering::Relaxed) {
                return CallbackProgression::Stop;
            }

            if attachment_id.has_event_from(&guard) {
                // Drain notification kinds (we don't strictly need to distinguish,
                // since we check all channels anyway)
                let _ = self.wakeup_listener.drain();
            }

            // Always check all channels on any wakeup
            self.drain_tasks();
            self.drain_daemon_events();
            self.drain_result_subscriptions();

            if self.shutdown.load(Ordering::Relaxed) {
                return CallbackProgression::Stop;
            }

            CallbackProgression::Continue
        });

        if let Err(e) = result {
            error!(plugin = %self.meta.name(), error = ?e, "WaitSet error");
        }

        // Shutdown
        if let Err(e) = self.plugin.on_stop() {
            error!(plugin = %self.meta.name(), error = %e, "on_stop error");
        }

        let header = EventHeader {
            event_type: EventKind::PluginStopped as u16,
            source_plugin_id: 0,
            associated_id: 0,
            _reserved: [0; 6],
        };
        if let Err(e) = self.plugin_event_pub.emit_signal(&header) {
            error!(plugin = %self.meta.name(), error = %e, "Failed to emit PluginStopped event");
        }

        info!(plugin = %self.meta.name(), "Plugin runtime exited");
        Ok(())
    }

    fn drain_tasks(&self) {
        while let Ok(Some(active_request)) = self.task_server.receive() {
            self.handle_task(active_request);
        }
    }

    fn drain_daemon_events(&self) {
        while let Ok(Some(event)) = self.daemon_event_sub.try_recv() {
            let kind = EventKind::try_from(event.header.event_type);
            debug!(?kind, "Daemon event received");

            if matches!(kind, Ok(EventKind::DaemonShutdown)) {
                self.shutdown.store(true, Ordering::Relaxed);
                return;
            }

            if let Ok(event) = event.to_event()
                && let Err(e) = self.plugin.on_event(event)
            {
                error!(plugin = %self.meta.name(), error = %e, "on_event error");
            }
        }
    }

    fn drain_result_subscriptions(&self) {
        for (source_id, subscriber) in &self.result_subscriptions {
            while let Ok(Some(result)) = subscriber.try_recv() {
                let result_name = extract_result_name(&result.header, &result.payload);

                // Deliver as a lightweight PluginResultAvailable event.
                // The actual data is accessible through the result subscriber
                // at the macro-generated handler level (lazy access).
                let event = Event::PluginResultAvailable {
                    source: source_id.clone(),
                    result_name,
                };

                if let Err(e) = self.plugin.on_event(event) {
                    error!(
                        plugin = %self.meta.name(),
                        source = %source_id,
                        error = %e,
                        "on_event (result available) error"
                    );
                }
            }
        }
    }

    fn handle_task(&self, active_request: ActiveTaskRequest) {
        let header = active_request.header();
        let task_id = header.task_id;

        let payload_bytes = active_request.payload();
        let request: TaskRequestPayloadOwned = match postcard::from_bytes(payload_bytes) {
            Ok(r) => r,
            Err(e) => {
                error!(task_id, error = %e, "Failed to deserialize task request");
                let resp = TaskResponseHeader {
                    task_id,
                    kind: ResponseKind::Error as u8,
                    flags: FLAG_IS_FINAL,
                    ..Default::default()
                };
                let msg = format!("deserialization error: {e}");
                if let Err(e) = active_request.send_response(&resp, msg.as_bytes()) {
                    error!(task_id, error = %e, "Failed to send error response to daemon");
                }
                return;
            }
        };

        let config: HashMap<String, String> = request.config.into_iter().collect();

        let (result_tx, mut result_rx) = tokio::sync::mpsc::channel(RESULT_CHANNEL_CAPACITY);

        let emitter: Arc<dyn TransportEmitter + Send + Sync> = Arc::new(());
        let task_ctx = Context::new(
            task_id,
            PathBuf::from(&request.sample_path),
            config,
            emitter,
            Some(result_tx),
            #[cfg(feature = "guest")]
            None,
        );

        let result = self.plugin.on_task(&task_ctx);
        task_ctx.close_result_channel();

        while let Ok(msg) = result_rx.try_recv() {
            forward_result_to_ipc(&active_request, &msg);
        }

        match result {
            Ok(()) => {
                let resp = TaskResponseHeader {
                    task_id,
                    flags: FLAG_IS_FINAL,
                    ..Default::default()
                };
                if let Err(e) = active_request.send_response(&resp, &[]) {
                    error!(task_id, error = %e, "Failed to send task completion to daemon");
                }

                let ev = EventHeader {
                    event_type: EventKind::TaskCompleted as u16,
                    source_plugin_id: 0,
                    associated_id: task_id,
                    _reserved: [0; 6],
                };
                if let Err(e) = self.plugin_event_pub.emit_signal(&ev) {
                    error!(task_id, error = %e, "Failed to emit TaskCompleted event");
                }
                info!(plugin = %self.meta.name(), task_id, "Task completed");
            }
            Err(e) => {
                let resp = TaskResponseHeader {
                    task_id,
                    kind: ResponseKind::Error as u8,
                    flags: FLAG_IS_FINAL,
                    ..Default::default()
                };
                let msg = e.to_string();
                if let Err(send_err) = active_request.send_response(&resp, msg.as_bytes()) {
                    error!(task_id, error = %send_err, "Failed to send task error to daemon");
                }

                let ev = EventHeader {
                    event_type: EventKind::TaskFailed as u16,
                    source_plugin_id: 0,
                    associated_id: task_id,
                    _reserved: [0; 6],
                };
                if let Err(emit_err) = self.plugin_event_pub.emit_signal(&ev) {
                    error!(task_id, error = %emit_err, "Failed to emit TaskFailed event");
                }
                error!(plugin = %self.meta.name(), task_id, error = %e, "Task failed");
            }
        }
    }
}

use crate::context::message::{ResultKind as MsgResultKind, TaskResultMessage};
use malbox_plugin_transport::ipc::headers::ResultFormat as IpcResultFormat;

fn forward_result_to_ipc(active_request: &ActiveTaskRequest, msg: &TaskResultMessage) {
    let (kind, ipc_format) = match msg.kind {
        MsgResultKind::Progress => (ResponseKind::Progress, IpcResultFormat::Json),
        MsgResultKind::Result => (
            ResponseKind::Result,
            match msg.format {
                crate::context::message::ResultFormat::Json => IpcResultFormat::Json,
                _ => IpcResultFormat::Bytes,
            },
        ),
        MsgResultKind::ResultRef => {
            debug!(
                task_id = msg.task_id,
                "ResultRef not supported for host plugins, skipping"
            );
            return;
        }
    };

    let name_bytes = msg.result_name.as_bytes();
    let name_len = name_bytes.len().min(255) as u8;

    let mut payload = Vec::with_capacity(name_len as usize + msg.data.len());
    payload.extend_from_slice(&name_bytes[..name_len as usize]);
    payload.extend_from_slice(&msg.data);

    let header = TaskResponseHeader {
        task_id: msg.task_id,
        kind: kind as u8,
        format: ipc_format as u8,
        name_len,
        ..Default::default()
    };

    if let Err(e) = active_request.send_response(&header, &payload) {
        debug!(task_id = msg.task_id, error = %e, "failed to forward result via IPC");
    }
}

fn extract_result_name(header: &ResultHeader, payload: &[u8]) -> String {
    let name_len = header.name_len as usize;
    if name_len > 0 && payload.len() >= name_len {
        String::from_utf8_lossy(&payload[..name_len]).to_string()
    } else {
        String::new()
    }
}
