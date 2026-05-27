use std::sync::Arc;
use std::time::Instant;

use dashmap::DashMap;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace, warn};

use crate::manager::instance::{PluginInstance, PluginLifecycle};
use crate::manager::runtime::PluginRuntime;
use crate::registry::types::PluginId;

pub fn spawn_health_check_loop(
    instances: Arc<DashMap<PluginId, Arc<Mutex<PluginInstance>>>>,
    interval: Duration,
    token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {}
                _ = token.cancelled() => {
                    debug!("health check loop: shutdown signal received, exiting");
                    break;
                }
            }

            for entry in instances.iter() {
                let plugin_id = entry.key();
                let instance_lock = entry.value();

                // Use try_lock: if the lock is held, the plugin is actively
                // executing a task (the handle holds the lock during
                // execute_task). A contended lock means the plugin is alive
                // by definition, so skipping is correct.
                let mut instance = match instance_lock.try_lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        trace!(plugin = %plugin_id, "lock contended, skipping health check");
                        continue;
                    }
                };

                if instance.lifecycle.is_starting() {
                    if let PluginRuntime::Host(_) = &instance.runtime
                        && instance.runtime.check_health().await
                    {
                        debug!(plugin = %plugin_id, "host plugin process alive, promoting to Ready");
                        instance.lifecycle = PluginLifecycle::Ready;
                    }
                    continue;
                }

                if !instance.lifecycle.is_running() {
                    continue;
                }

                let healthy = instance.runtime.check_health().await;

                if !healthy {
                    let kind = match &instance.runtime {
                        PluginRuntime::Host(_) => "host",
                        PluginRuntime::Guest(_) => "guest",
                    };
                    let reason = format!("{} plugin '{}' failed health check", kind, plugin_id);
                    warn!(reason = %reason, "Plugin failed health check");
                    instance.lifecycle = PluginLifecycle::Failed { reason };
                }

                instance.last_health_check = Some(Instant::now());
            }

            trace!(
                "health check cycle complete for {} plugin(s)",
                instances.len()
            );
        }
    })
}
