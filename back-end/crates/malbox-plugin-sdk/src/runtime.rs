//! Plugin runtime — sets up IPC channels and runs the event loop.

use crate::error::{Result, SdkError};
use crate::plugin::{EventContext, Plugin};

use malbox_plugin_transport::ipc::{
    daemon_channel, plugin_channel, EventEmitter, EventReceiver, IpcService, NodeBuilder,
};
use malbox_plugin_transport::messages::events::*;
use malbox_plugin_transport::traits::TransportReceiver;

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{debug, error, info, warn};

/// Configuration for the plugin runtime.
pub struct RuntimeConfig {
    /// How long to wait for events before checking the shutdown flag.
    pub poll_interval: Duration,

    /// Whether to automatically emit PluginStarted/PluginStopped events.
    pub auto_lifecycle_events: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(100),
            auto_lifecycle_events: true,
        }
    }
}

/// Manages IPC channels and the event dispatch loop for a plugin.
pub struct PluginRuntime<P: Plugin> {
    plugin: P,
    receiver: EventReceiver,
    emitter: EventEmitter,
    config: RuntimeConfig,
    shutdown: AtomicBool,
}

impl<P: Plugin> PluginRuntime<P> {
    /// Create a new plugin runtime with default configuration.
    pub fn new(plugin: P) -> Result<Self> {
        Self::with_config(plugin, RuntimeConfig::default())
    }

    /// Create a new plugin runtime with custom configuration.
    pub fn with_config(plugin: P, config: RuntimeConfig) -> Result<Self> {
        info!("Initializing plugin runtime for '{}'", plugin.name());

        let node = NodeBuilder::new()
            .create::<IpcService>()
            .map_err(|e| SdkError::Init(format!("Failed to create IPC node: {}", e)))?;

        let receiver =
            EventReceiver::new(&node, daemon_channel::EVENTS, daemon_channel::PAYLOADS)
                .map_err(|e| {
                    SdkError::Init(format!("Failed to create event receiver: {}", e))
                })?;

        let emitter =
            EventEmitter::new(&node, plugin_channel::EVENTS, plugin_channel::PAYLOADS)
                .map_err(|e| {
                    SdkError::Init(format!("Failed to create event emitter: {}", e))
                })?;

        info!("Plugin runtime for '{}' initialized", plugin.name());

        Ok(Self {
            plugin,
            receiver,
            emitter,
            config,
            shutdown: AtomicBool::new(false),
        })
    }

    /// Run the plugin event loop. Blocks until shutdown.
    pub fn run(&self) -> Result<()> {
        let ctx = EventContext::new(&self.emitter);
        let plugin_id = self.plugin.plugin_id();

        self.plugin.on_start(&ctx)?;

        if self.config.auto_lifecycle_events {
            ctx.emit_plugin_started(plugin_id)?;
            info!("Emitted PluginStarted for '{}'", self.plugin.name());
        }

        info!("Plugin '{}' entering event loop", self.plugin.name());

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                info!("Shutdown signal received for '{}'", self.plugin.name());
                break;
            }

            match self.receiver.wait(self.config.poll_interval) {
                Ok(Some((event, payload))) => {
                    debug!("Received event: {:?}", event);
                    if let Err(e) = self.dispatch_event(event, payload, &ctx) {
                        error!("Handler error in '{}': {}", self.plugin.name(), e);
                    }
                }
                Ok(None) => continue,
                Err(e) => {
                    error!("Transport error in '{}': {}", self.plugin.name(), e);
                }
            }
        }

        self.plugin.on_stop(&ctx)?;

        if self.config.auto_lifecycle_events {
            ctx.emit_plugin_stopped(plugin_id)?;
            info!("Emitted PluginStopped for '{}'", self.plugin.name());
        }

        info!("Plugin '{}' runtime exited", self.plugin.name());
        Ok(())
    }

    fn dispatch_event(&self, event: Event, payload: Payload, ctx: &EventContext) -> Result<()> {
        match (event, payload) {
            (Event::Task(task_event), Payload::Task(task_payload)) => {
                self.plugin.on_task_event(task_event, task_payload, ctx)
            }
            (Event::Plugin(plugin_event), Payload::Plugin(plugin_payload)) => {
                self.plugin
                    .on_plugin_event(plugin_event, plugin_payload, ctx)
            }
            (Event::Sample(sample_event), Payload::Sample(sample_payload)) => {
                self.plugin
                    .on_sample_event(sample_event, sample_payload, ctx)
            }
            (Event::Daemon(daemon_event), _) => {
                if matches!(daemon_event, DaemonEvent::DaemonShutdown) {
                    self.shutdown.store(true, Ordering::Relaxed);
                }
                self.plugin.on_daemon_event(daemon_event, ctx)
            }
            (event, payload) => {
                warn!("Event/payload category mismatch: {:?} with {:?}", event, payload);
                Ok(())
            }
        }
    }
}
