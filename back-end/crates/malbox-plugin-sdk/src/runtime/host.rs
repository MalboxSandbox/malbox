//! Host plugin runtime — IPC-based event loop.
//!
//! Host plugins run on the same machine as the daemon and communicate via
//! iceoryx2 shared-memory IPC. The runtime polls the receiver for incoming
//! events and dispatches them to the plugin.
//!
//! Gated behind the `host` feature flag.

use crate::context::Context;
use crate::error::{Result, SdkError};
use crate::plugin::Plugin;
use crate::types::PluginMeta;

use malbox_plugin_transport::ipc::{
    EventEmitter, EventReceiver, IpcService, NodeBuilder, daemon_channel, plugin_channel,
};
use malbox_plugin_transport::messages::events::Event;
use malbox_plugin_transport::traits::TransportReceiver;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tracing::{debug, error, info};

/// Host plugin runtime.
pub struct HostRuntime<P> {
    plugin: P,
    meta: PluginMeta,
    receiver: EventReceiver,
    emitter: EventEmitter,
    shutdown: AtomicBool,
}

impl<P: Plugin> HostRuntime<P> {
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
        let ctx = Context::new(&self.emitter, None, None);

        // Call on_start
        self.plugin.on_start(HashMap::new())?;

        // Emit PluginStarted
        ctx.emit_event(Event::PluginStarted { plugin_id: 0 })?;
        info!("Plugin '{}' started, entering event loop", self.meta.name);

        // Event loop
        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                info!("Shutdown signal received for '{}'", self.meta.name);
                break;
            }

            match self.receiver.wait(Duration::from_millis(100)) {
                Ok(Some(event)) => {
                    debug!("Received event: {:?}", event);
                    if matches!(event, Event::DaemonShutdown) {
                        self.shutdown.store(true, Ordering::Relaxed);
                    }
                    if let Err(e) = self.plugin.on_event(event, &ctx) {
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
        if let Err(e) = self.plugin.on_stop() {
            error!("on_stop error in '{}': {}", self.meta.name, e);
        }

        // Emit PluginStopped
        let _ = ctx.emit_event(Event::PluginStopped { plugin_id: 0 });

        info!("Plugin '{}' runtime exited", self.meta.name);
        Ok(())
    }
}
