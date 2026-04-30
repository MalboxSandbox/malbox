//! The GuestPlugin trait for malbox guest (in-VM) plugins.

use crate::context::Context;
use crate::error::Result;
use crate::types::{HealthStatus, Task};
use std::path::Path;

/// Launch a sample using the platform-native process creation API.
///
/// This is the default implementation of [`GuestPlugin::execute_sample`].
pub fn default_launch(sample_path: &Path) -> bool {
    use tracing::{error, info};

    if !sample_path.exists() {
        error!(path = %sample_path.display(), "default_launch: sample file does not exist");
        return false;
    }

    info!(path = %sample_path.display(), "default_launch: launching sample");

    #[cfg(target_os = "windows")]
    let result = {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_CONSOLE: u32 = 0x00000010;
        std::process::Command::new(sample_path)
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
    };

    #[cfg(not(target_os = "windows"))]
    let result = std::process::Command::new(sample_path).spawn();

    match result {
        Ok(mut child) => {
            let pid = child.id();
            info!(
                path = %sample_path.display(),
                pid,
                "default_launch: sample launched successfully"
            );

            std::thread::spawn(move || {
                match child.wait() {
                    Ok(status) => {
                        tracing::info!(
                            pid,
                            exit_code = status.code(),
                            success = status.success(),
                            "default_launch: sample process exited"
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            pid,
                            error = %e,
                            "default_launch: failed to wait on sample process"
                        );
                    }
                }
            });

            true
        }
        Err(e) => {
            error!(
                path = %sample_path.display(),
                error = %e,
                kind = ?e.kind(),
                "default_launch: failed to launch sample"
            );
            false
        }
    }
}

/// Trait for malbox guest plugins that run inside an ephemeral VM.
///
/// The SDK owns the lifecycle sequence:
/// 1. `on_start` - plugin sets up monitoring, receives task info and context
/// 2. `execute_sample` - SDK calls this to launch the sample (default: platform launcher)
/// 3. SDK waits for analysis timeout
/// 4. `on_stop` - plugin flushes results and tears down
pub trait GuestPlugin: Send + Sync + 'static {
    /// Called when the analysis task begins. Set up monitoring infrastructure
    /// (ETW sessions, decoders, sinks, etc.) and return when ready to capture.
    ///
    /// The SDK guarantees `execute_sample` will not be called until this returns.
    fn on_start(&self, task: &Task, ctx: &Context) -> Result<()>;

    /// Called when the analysis timeout expires or the daemon signals shutdown.
    /// Flush all buffered results via [`Context::push_result`] and tear down.
    fn on_stop(&self, ctx: &Context) -> Result<()>;

    /// Launch the sample at the given path. Called by the SDK after `on_start`.
    ///
    /// The default implementation uses the platform's process creation API.
    /// Override for non-EXE scenarios (DLL loading, COM dispatch, etc.).
    fn execute_sample(&self, sample_path: &Path) -> bool {
        default_launch(sample_path)
    }

    /// Return the plugin's current health status.
    fn health_check(&self) -> HealthStatus {
        HealthStatus::ready()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MinimalGuestPlugin;
    impl GuestPlugin for MinimalGuestPlugin {
        fn on_start(&self, _task: &Task, _ctx: &Context) -> Result<()> {
            Ok(())
        }
        fn on_stop(&self, _ctx: &Context) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn minimal_guest_plugin_defaults_work() {
        let plugin = MinimalGuestPlugin;
        let health = plugin.health_check();
        assert!(health.is_ready());
    }

    #[test]
    fn execute_sample_has_default_impl() {
        let plugin = MinimalGuestPlugin;
        let _ = &plugin as &dyn GuestPlugin;
    }
}
