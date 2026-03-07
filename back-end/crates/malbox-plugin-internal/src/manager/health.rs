//! Background health-check loop for running plugin instances.
//!
//! [`spawn_health_check_loop`] starts a `tokio::spawn` task that periodically
//! inspects every running plugin instance. Host plugins are checked by polling
//! `Child::try_wait()` to detect unexpected exits; guest plugins are checked
//! via a gRPC `health_check` RPC with a 5-second timeout. Any plugin that
//! fails its health check is transitioned to [`PluginLifecycle::Failed`].

use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tracing::{debug, warn};

use crate::manager::instance::{PluginInstance, PluginLifecycle};
use crate::registry::manifest::PluginTypeConfig;
use crate::registry::types::PluginId;

/// Spawn a background task that periodically health-checks all running plugins.
///
/// The task loops on a fixed `interval`, checking every entry in `instances`.
/// It exits cleanly when `shutdown_rx` receives a `true` value.
pub fn spawn_health_check_loop(
    instances: Arc<DashMap<PluginId, Arc<Mutex<PluginInstance>>>>,
    interval: Duration,
    mut shutdown_rx: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {}
                result = shutdown_rx.changed() => {
                    // Channel closed or shutdown requested.
                    if result.is_err() || *shutdown_rx.borrow() {
                        debug!("health check loop: shutdown signal received, exiting");
                        break;
                    }
                }
            }

            for entry in instances.iter() {
                let plugin_id = entry.key();
                let instance_lock = entry.value();
                let mut instance = instance_lock.lock().await;

                if !instance.lifecycle.is_running() {
                    continue;
                }

                let plugin_type = instance.entry.manifest.plugin.plugin_type;
                let healthy = match plugin_type {
                    PluginTypeConfig::Host => check_host_health(&mut instance).await,
                    PluginTypeConfig::Guest => check_guest_health(&mut instance).await,
                };

                if !healthy {
                    let reason = format!(
                        "{} plugin '{}' failed health check",
                        match plugin_type {
                            PluginTypeConfig::Host => "host",
                            PluginTypeConfig::Guest => "guest",
                        },
                        plugin_id
                    );
                    warn!("{}", reason);
                    instance.lifecycle = PluginLifecycle::Failed { reason };
                }

                instance.last_health_check = Some(Instant::now());
            }

            debug!("health check cycle complete for {} plugin(s)", instances.len());
        }
    })
}

/// Check whether a host plugin's child process is still alive.
///
/// Uses `Child::try_wait()` which is non-blocking:
/// - `Ok(None)` means the process has not exited yet (healthy).
/// - `Ok(Some(_))` means the process exited (unhealthy).
/// - `Err(_)` means we failed to query the process (unhealthy).
///
/// Returns `false` if `instance.process` is `None`, which should not happen
/// for a host plugin in a running lifecycle state.
async fn check_host_health(instance: &mut PluginInstance) -> bool {
    match instance.process {
        Some(ref mut child) => match child.try_wait() {
            Ok(None) => true,
            Ok(Some(_)) => false,
            Err(_) => false,
        },
        None => false,
    }
}

/// Check whether a guest plugin is responsive via gRPC health check.
///
/// Calls `GrpcClient::health_check()` with a 5-second timeout.
async fn check_guest_health(instance: &mut PluginInstance) -> bool {
    let client = match instance.grpc_client {
        Some(ref mut c) => c,
        None => return false,
    };

    let timeout = Duration::from_secs(5);
    match tokio::time::timeout(timeout, client.health_check()).await {
        Ok(Ok(_)) => true,
        Ok(Err(_)) => false,
        Err(_) => false,
    }
}
