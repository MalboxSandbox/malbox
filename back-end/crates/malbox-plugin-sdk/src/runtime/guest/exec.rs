//! Default command executor for the guest runtime.
//!
//! When a plugin's `Plugin::on_execute_command` returns `None`, the bridge
//! falls back to [`default_execute_command`] which spawns the requested
//! OS command via `tokio::process::Command`.
//!
//! The execution synchronization channel ([`ExecutionNotifier`] /
//! [`ExecutionWaiter`]) lives in [`crate::execution`] because it is shared
//! with the feature-agnostic [`Context`](crate::context::Context).

pub use crate::execution::{ExecutionNotifier, ExecutionWaiter, execution_channel};

use malbox_plugin_transport::grpc::proto;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use super::files::resolve_path;

/// Spawn an OS command using `tokio::process::Command`.
///
/// This is the fallback executor used when `Plugin::on_execute_command`
/// returns `None`. The `cwd` is resolved against `base_dir` for safety.
pub(super) async fn default_execute_command(
    base_dir: &Path,
    command: &str,
    args: &[String],
    cwd: Option<&str>,
    env: HashMap<String, String>,
    timeout_ms: Option<u64>,
    background: bool,
) -> std::result::Result<proto::ExecResponse, String> {
    let mut cmd = tokio::process::Command::new(command);
    cmd.args(args).envs(&env);

    if let Some(cwd) = cwd {
        cmd.current_dir(resolve_path(base_dir, cwd)?);
    }

    if background {
        cmd.stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let child = cmd.spawn().map_err(|e| format!("failed to spawn: {}", e))?;
        let pid = child.id();
        return Ok(proto::ExecResponse {
            exit_code: None,
            stdout: vec![],
            stderr: vec![],
            pid,
        });
    }

    let output_fut = cmd.output();
    let output = match timeout_ms {
        Some(ms) => tokio::time::timeout(Duration::from_millis(ms), output_fut)
            .await
            .map_err(|_| "command timed out".to_string())?
            .map_err(|e| format!("command failed: {}", e))?,
        None => output_fut
            .await
            .map_err(|e| format!("command failed: {}", e))?,
    };

    Ok(proto::ExecResponse {
        exit_code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
        pid: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ExecutionNotifier / ExecutionWaiter tests live in `crate::execution`.

    #[tokio::test]
    async fn execute_command_captures_output() {
        let dir = tempfile::tempdir().unwrap();
        let result = default_execute_command(
            dir.path(),
            "echo",
            &["hello".to_string()],
            None,
            HashMap::new(),
            None,
            false,
        )
        .await;

        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.exit_code, Some(0));
        assert_eq!(String::from_utf8_lossy(&resp.stdout).trim(), "hello");
        assert_eq!(resp.pid, None);
    }

    #[tokio::test]
    async fn execute_command_respects_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let result = default_execute_command(
            dir.path(),
            "sleep",
            &["10".to_string()],
            None,
            HashMap::new(),
            Some(50),
            false,
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("timed out"));
    }

    #[tokio::test]
    async fn execute_command_background_returns_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let result = default_execute_command(
            dir.path(),
            "sleep",
            &["60".to_string()],
            None,
            HashMap::new(),
            None,
            true,
        )
        .await;

        assert!(result.is_ok());
        let resp = result.unwrap();
        assert_eq!(resp.exit_code, None);
        assert!(resp.pid.is_some());
    }
}
