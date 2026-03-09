//! Host plugin runtime — IPC-based event loop using internal handler traits.
//!
//! This module is gated behind the `host` feature flag.

use crate::context::Context;
use crate::error::{Result, SdkError};
use crate::internal::{
    DaemonEventHandler, PluginEventHandler, SampleEventHandler, StartHandler, StopHandler,
    TaskEventHandler, TaskHandler,
};
use crate::types::{PluginMeta, Task};

use malbox_plugin_transport::ipc::{
    EventEmitter, EventReceiver, IpcService, NodeBuilder, daemon_channel, plugin_channel,
};
use malbox_plugin_transport::messages::events::*;
use malbox_plugin_transport::traits::TransportReceiver;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{debug, error, info, warn};

/// Host plugin runtime that uses the internal handler traits.
pub struct HostRuntime<P> {
    plugin: P,
    meta: PluginMeta,
    receiver: EventReceiver,
    emitter: EventEmitter,
    shutdown: AtomicBool,
}

impl<P> HostRuntime<P>
where
    P: TaskHandler
        + StartHandler
        + StopHandler
        + DaemonEventHandler
        + TaskEventHandler
        + PluginEventHandler
        + SampleEventHandler,
{
    /// Create a new host plugin runtime.
    pub fn new(plugin: P, meta: PluginMeta) -> Result<Self> {
        info!("Initializing host runtime for '{}'", meta.name);

        let node = NodeBuilder::new()
            .create::<IpcService>()
            .map_err(|e| SdkError::Init(format!("Failed to create IPC node: {}", e)))?;

        let receiver = EventReceiver::new(&node, daemon_channel::EVENTS, daemon_channel::PAYLOADS)
            .map_err(|e| SdkError::Init(format!("Failed to create event receiver: {}", e)))?;

        let emitter = EventEmitter::new(&node, plugin_channel::EVENTS, plugin_channel::PAYLOADS)
            .map_err(|e| SdkError::Init(format!("Failed to create event emitter: {}", e)))?;

        info!("Host runtime for '{}' initialized", meta.name);

        Ok(Self {
            plugin,
            meta,
            receiver,
            emitter,
            shutdown: AtomicBool::new(false),
        })
    }

    /// Run the plugin event loop. Blocks until shutdown.
    pub fn run(&self) -> Result<()> {
        let ctx = Context::new(&self.emitter);

        // Call on_start
        self.plugin.__handle_start(HashMap::new())?;

        // Emit PluginStarted
        ctx.emit_event(
            Event::Plugin(PluginEvent::PluginStarted),
            Payload::Plugin(PluginEventPayload { plugin_id: 0 }),
        )?;
        info!("Plugin '{}' started, entering event loop", self.meta.name);

        // Event loop
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                info!("Shutdown signal received for '{}'", self.meta.name);
                break;
            }

            match self.receiver.wait(Duration::from_millis(100)) {
                Ok(Some((event, payload))) => {
                    debug!("Received event: {:?}", event);
                    if let Err(e) = self.dispatch_event(event, payload, &ctx) {
                        error!("Handler error in '{}': {}", self.meta.name, e);
                    }
                }
                Ok(None) => continue,
                Err(e) => {
                    error!("Transport error in '{}': {}", self.meta.name, e);
                }
            }
        }

        // Call on_stop
        if let Err(e) = self.plugin.__handle_stop() {
            error!("on_stop error in '{}': {}", self.meta.name, e);
        }

        // Emit PluginStopped
        let _ = ctx.emit_event(
            Event::Plugin(PluginEvent::PluginStopped),
            Payload::Plugin(PluginEventPayload { plugin_id: 0 }),
        );

        info!("Plugin '{}' runtime exited", self.meta.name);
        Ok(())
    }

    /// Route an incoming event/payload pair to the appropriate handler trait method.
    fn dispatch_event(&self, event: Event, payload: Payload, ctx: &Context) -> Result<()> {
        match (event, payload) {
            (Event::Task(task_event), Payload::Task(task_payload)) => {
                // For task-starting events, trigger the task handler
                if matches!(task_event, TaskEvent::TaskStarting | TaskEvent::TaskCreated) {
                    let task = Task::new(
                        task_payload.task_id,
                        std::path::PathBuf::new(), // Host plugins get path from config
                        HashMap::new(),
                    );
                    let results = self.plugin.__handle_task(task, ctx)?;
                    for result in &results {
                        debug!("Produced result: {}", result.name());
                    }
                    // TODO: stream results back to daemon
                }
                // Always notify the lifecycle event handler
                self.plugin
                    .__handle_task_lifecycle_event(task_event, task_payload, ctx)
            }
            (Event::Plugin(plugin_event), Payload::Plugin(plugin_payload)) => {
                self.plugin
                    .__handle_plugin_event(plugin_event, plugin_payload, ctx)
            }
            (Event::Sample(sample_event), Payload::Sample(sample_payload)) => {
                self.plugin
                    .__handle_sample_event(sample_event, sample_payload, ctx)
            }
            (Event::Daemon(daemon_event), _) => {
                if matches!(daemon_event, DaemonEvent::DaemonShutdown) {
                    self.shutdown.store(true, Ordering::Relaxed);
                }
                self.plugin.__handle_daemon_event(daemon_event, ctx)
            }
            (event, payload) => {
                warn!("Unhandled event/payload: {:?} / {:?}", event, payload);
                Ok(())
            }
        }
    }
}
