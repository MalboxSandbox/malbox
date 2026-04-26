//! Plugin lifecycle manager.
//!
//! The [`PluginManager`] owns all running plugin instances, coordinates startup
//! and shutdown, acquires handles for task execution, and drives background
//! health checks. It is the central orchestrator that sits between the registry
//! (static discovery) and the scheduler (dynamic task assignment).

pub mod error;
pub mod handle;
pub mod health;
pub mod instance;
pub mod log_router;

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use tokio::sync::{Mutex, watch};
use tokio::task::JoinHandle;
use tracing::{debug, info, instrument, warn};

use crate::manager::error::{ManagerError, Result};
use crate::manager::handle::PluginHandle;
use crate::manager::health::spawn_health_check_loop;
use crate::manager::instance::{PluginInstance, PluginLifecycle};
use crate::registry::PluginRegistry;
use crate::registry::manifest::{PluginStateConfig, PluginTypeConfig};
use crate::registry::types::{PluginEntry, PluginId};
use crate::transport::ipc::EventEmitter;

/// Central plugin lifecycle manager.
///
/// Owns all running plugin instances and coordinates their lifecycle:
/// spawning, health checking, acquisition for task execution, and shutdown.
///
/// The `instances` map uses [`DashMap`] for concurrent access so that methods
/// like [`register_guest`] and [`reconcile`] can insert/remove entries through
/// `&self` without requiring `&mut self`.
pub struct PluginManager {
    instances: Arc<DashMap<PluginId, Arc<Mutex<PluginInstance>>>>,
    registry: Arc<PluginRegistry>,
    emitter: Arc<EventEmitter>,
    /// Held to keep the background health-check task alive for the lifetime of the manager.
    #[allow(dead_code)]
    health_check_handle: JoinHandle<()>,
    shutdown_tx: watch::Sender<bool>,
}

impl PluginManager {
    /// Create a new plugin manager, spawning all persistent host plugins and
    /// starting the background health check loop.
    #[instrument(skip_all, err)]
    pub async fn new(
        registry: Arc<PluginRegistry>,
        emitter: Arc<EventEmitter>,
        health_check_interval: Duration,
    ) -> Result<Self> {
        let snapshot = registry.snapshot();
        let instances: Arc<DashMap<PluginId, Arc<Mutex<PluginInstance>>>> =
            Arc::new(DashMap::new());

        // Spawn all persistent host plugins at startup.
        for entry in snapshot.list() {
            let is_persistent = entry.manifest.runtime.state == PluginStateConfig::Persistent;
            let is_host = entry.manifest.plugin.plugin_type == PluginTypeConfig::Host;

            if is_persistent && is_host {
                info!(plugin = %entry.id, "spawning persistent host plugin");
                match spawn_host_plugin(entry).await {
                    Ok(instance) => {
                        instances.insert(entry.id.clone(), Arc::new(Mutex::new(instance)));
                    }
                    Err(e) => {
                        warn!(plugin = %entry.id, error = %e, "failed to spawn persistent plugin, skipping");
                    }
                }
            }
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let health_check_handle =
            spawn_health_check_loop(Arc::clone(&instances), health_check_interval, shutdown_rx);

        info!(count = instances.len(), "Plugin manager initialized");

        Ok(Self {
            instances,
            registry,
            emitter,
            health_check_handle,
            shutdown_tx,
        })
    }

    /// Acquire a handle to a plugin for task execution.
    /// The behaviour depends on the plugin's [`PluginStateConfig`]:
    ///
    /// - **Persistent**: looks up the already-running instance, checks that it
    ///   is [`Ready`](PluginLifecycle::Ready), transitions it to `Busy`, and
    ///   returns a handle.
    /// - **Ephemeral**: spawns a fresh process, marks it `Ready` then `Busy`,
    ///   and returns a handle. The handle owns the instance — it is not inserted
    ///   into `self.instances`.
    /// - **Scoped**: not yet implemented.
    pub async fn acquire(&self, plugin_id: &PluginId) -> Result<PluginHandle> {
        let snapshot = self.registry.snapshot();
        let entry = snapshot
            .get(plugin_id)
            .ok_or_else(|| ManagerError::PluginNotFound(plugin_id.clone()))?;

        match entry.manifest.runtime.state {
            PluginStateConfig::Persistent => {
                let instance_lock = self
                    .instances
                    .get(plugin_id)
                    .ok_or_else(|| ManagerError::PluginNotFound(plugin_id.clone()))?;

                let mut instance = instance_lock.value().lock().await;
                if !instance.lifecycle.can_acquire() {
                    return Err(ManagerError::PluginBusy(plugin_id.clone()));
                }

                instance.lifecycle = PluginLifecycle::Busy { task_id: 0 };

                Ok(PluginHandle::new(
                    plugin_id.clone(),
                    Arc::clone(entry),
                    Arc::clone(instance_lock.value()),
                ))
            }
            PluginStateConfig::Ephemeral => {
                // Guest plugins registered via register_guest() are already in
                // the instances map with a gRPC client. Use the existing
                // instance instead of trying to spawn a local process.
                if let Some(instance_lock) = self.instances.get(plugin_id) {
                    let mut instance = instance_lock.value().lock().await;
                    if instance.grpc_client.is_some() && instance.lifecycle.can_acquire() {
                        instance.lifecycle = PluginLifecycle::Busy { task_id: 0 };
                        return Ok(PluginHandle::new(
                            plugin_id.clone(),
                            Arc::clone(entry),
                            Arc::clone(instance_lock.value()),
                        ));
                    }
                }

                info!(plugin = %plugin_id, "spawning ephemeral plugin instance");
                let mut instance = spawn_host_plugin(entry).await?;
                instance.lifecycle = PluginLifecycle::Ready;
                instance.lifecycle = PluginLifecycle::Busy { task_id: 0 };

                let instance_lock = Arc::new(Mutex::new(instance));

                Ok(PluginHandle::new(
                    plugin_id.clone(),
                    Arc::clone(entry),
                    instance_lock,
                ))
            }
            PluginStateConfig::Scoped => Err(ManagerError::ScopedNotImplemented),
        }
    }

    /// Register a guest plugin that connected from inside a VM.
    ///
    /// Establishes a gRPC client connection to the given address and creates a
    /// [`PluginInstance`] in the [`Ready`](PluginLifecycle::Ready) state. The
    /// instance is inserted into the instances map so it can be acquired by
    /// workers for task execution.
    #[instrument(skip_all, fields(plugin = %plugin_id, addr = %addr), err)]
    pub async fn register_guest(&self, plugin_id: &PluginId, addr: String) -> Result<()> {
        let snapshot = self.registry.snapshot();
        let entry = snapshot
            .get(plugin_id)
            .ok_or_else(|| ManagerError::PluginNotFound(plugin_id.clone()))?;

        let mut grpc_client = crate::transport::daemon::GrpcClient::connect(&addr)
            .await
            .map_err(ManagerError::Transport)?;

        // Initialize the guest plugin (triggers on_start, decoder/provider registration).
        let init_resp = grpc_client
            .initialize(0, std::collections::HashMap::new())
            .await
            .map_err(ManagerError::Transport)?;

        if !init_resp.success {
            return Err(ManagerError::ExecutionFailed(
                plugin_id.clone(),
                format!(
                    "guest plugin initialization failed: {}",
                    init_resp.error_message
                ),
            ));
        }

        let instance = PluginInstance {
            entry: Arc::clone(entry),
            lifecycle: PluginLifecycle::Ready,
            process: None,
            grpc_client: Some(grpc_client),
            started_at: Some(Instant::now()),
            last_health_check: None,
            log_file_path: None,
        };

        self.instances
            .insert(plugin_id.clone(), Arc::new(Mutex::new(instance)));

        // Start consuming the guest plugin's log stream in the background.
        {
            let instance_lock = self
                .instances
                .get(plugin_id)
                .expect("instance was just inserted");
            let mut instance = instance_lock.value().lock().await;

            if let Some(ref mut client) = instance.grpc_client {
                match client.stream_logs(true).await {
                    Ok(stream) => {
                        let run_id = format!(
                            "{}",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis()
                        );
                        let log_dir = std::path::PathBuf::from("logs");
                        let (_handle, log_path) = log_router::spawn_log_consumer(
                            plugin_id.clone(),
                            stream,
                            log_dir,
                            run_id,
                        );
                        instance.log_file_path = Some(log_path);
                        debug!(plugin = %plugin_id, "log stream consumer started");
                    }
                    Err(e) => {
                        warn!(
                            plugin = %plugin_id,
                            error = %e,
                            "failed to start log stream (non-fatal)"
                        );
                    }
                }
            }
        }

        info!(plugin = %plugin_id, addr = %addr, "guest plugin registered");

        Ok(())
    }

    /// Remove a previously registered guest plugin instance.
    ///
    /// Called after task execution to clean up stale gRPC connections when the
    /// VM that hosted the plugin is about to be reverted or destroyed.
    pub fn unregister_guest(&self, plugin_id: &PluginId) {
        if self.instances.remove(plugin_id).is_some() {
            info!(plugin = %plugin_id, "guest plugin unregistered");
        }
    }

    /// Reconcile the running instances with the current registry state.
    ///
    /// - Stops instances whose plugins have been removed from the registry.
    /// - Spawns newly discovered persistent host plugins.
    pub async fn reconcile(&self) {
        let snapshot = self.registry.snapshot();

        // Check for removed plugins — stop instances that are no longer registered.
        let orphaned_ids: Vec<PluginId> = self
            .instances
            .iter()
            .filter(|entry| snapshot.get(entry.key()).is_none())
            .map(|entry| entry.key().clone())
            .collect();

        for plugin_id in &orphaned_ids {
            if let Some(instance_ref) = self.instances.get(plugin_id) {
                let mut instance = instance_ref.value().lock().await;
                if instance.lifecycle.is_running() {
                    warn!(plugin = %plugin_id, "plugin removed from registry, stopping instance");
                    instance.lifecycle = PluginLifecycle::Stopping;

                    if let Some(ref mut process) = instance.process {
                        let _ = process.kill().await;
                    }

                    instance.lifecycle = PluginLifecycle::Stopped;
                }
            }
            self.instances.remove(plugin_id);
        }

        // Spawn new persistent host plugins that should be running.
        for entry in snapshot.list() {
            let is_persistent = entry.manifest.runtime.state == PluginStateConfig::Persistent;
            let is_host = entry.manifest.plugin.plugin_type == PluginTypeConfig::Host;

            if is_persistent && is_host && !self.instances.contains_key(&entry.id) {
                info!(plugin = %entry.id, "spawning newly discovered persistent host plugin");
                match spawn_host_plugin(entry).await {
                    Ok(instance) => {
                        self.instances
                            .insert(entry.id.clone(), Arc::new(Mutex::new(instance)));
                    }
                    Err(e) => {
                        warn!(plugin = %entry.id, error = %e, "failed to spawn new persistent plugin");
                    }
                }
            }
        }
    }

    /// Shut down the plugin manager, stopping all running instances and the
    /// health check loop.
    pub async fn shutdown(&self) {
        info!("shutting down plugin manager");

        // Signal the health check loop to stop.
        let _ = self.shutdown_tx.send(true);

        // Stop all running plugin instances.
        for entry in self.instances.iter() {
            let plugin_id = entry.key();
            let mut instance = entry.value().lock().await;

            if instance.lifecycle.is_running() {
                debug!(plugin = %plugin_id, "stopping plugin instance");
                instance.lifecycle = PluginLifecycle::Stopping;

                if let Some(ref mut process) = instance.process {
                    let _ = process.kill().await;
                }

                instance.lifecycle = PluginLifecycle::Stopped;
            }
        }

        info!("all plugin instances stopped");
    }

    /// Returns a reference to the underlying plugin registry.
    pub fn registry(&self) -> &Arc<PluginRegistry> {
        &self.registry
    }

    /// Returns a reference to the IPC event emitter.
    pub fn emitter(&self) -> &Arc<EventEmitter> {
        &self.emitter
    }
}

/// Spawn a host plugin as a child process.
///
/// The returned instance is in the [`Starting`](PluginLifecycle::Starting)
/// state with the process handle attached.
async fn spawn_host_plugin(entry: &PluginEntry) -> Result<PluginInstance> {
    let child = tokio::process::Command::new(&entry.binary_path)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ManagerError::SpawnFailed(entry.id.clone(), e.to_string()))?;

    Ok(PluginInstance {
        entry: Arc::new(entry.clone()),
        lifecycle: PluginLifecycle::Starting,
        process: Some(child),
        grpc_client: None,
        started_at: Some(Instant::now()),
        last_health_check: None,
        log_file_path: None,
    })
}
