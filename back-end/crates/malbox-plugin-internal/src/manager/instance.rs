//! Plugin instance lifecycle and runtime state.
//!
//! Each running plugin is represented by a [`PluginInstance`] which tracks the
//! process handle, gRPC client connection (for guest plugins), and current
//! lifecycle state. The [`PluginLifecycle`] enum models the state machine that
//! governs what operations are valid at any point in time.

use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use crate::registry::types::PluginEntry;

/// Lifecycle state machine for a running plugin process.
///
/// Transitions follow a directed graph:
///
/// ```text
/// Starting -> Ready -> Busy -> Ready
///                |               |
///                v               v
///             Stopping        Stopping
///                |               |
///                v               v
///             Stopped         Stopped
///
/// Any state can transition to Failed on crash or timeout.
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginLifecycle {
    /// Process has been spawned; waiting for the ready signal (health check or
    /// gRPC handshake).
    Starting,
    /// Plugin is idle and available for task assignment.
    Ready,
    /// Plugin is currently executing a task.
    Busy {
        /// ID of the task being executed.
        task_id: i32,
    },
    /// Graceful shutdown has been requested; waiting for the process to exit.
    Stopping,
    /// Process has exited normally.
    Stopped,
    /// Plugin crashed, became unresponsive, or encountered an unrecoverable error.
    Failed {
        /// Human-readable description of what went wrong.
        reason: String,
    },
}

impl PluginLifecycle {
    /// Returns `true` if the plugin is in the [`Starting`](Self::Starting) state.
    pub fn is_starting(&self) -> bool {
        matches!(self, Self::Starting)
    }

    /// Returns `true` if the plugin is in the [`Ready`](Self::Ready) state.
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// Returns `true` if the plugin is in the [`Busy`](Self::Busy) state.
    pub fn is_busy(&self) -> bool {
        matches!(self, Self::Busy { .. })
    }

    /// Returns `true` if the plugin is in the [`Failed`](Self::Failed) state.
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Returns `true` if the plugin is actively running — either [`Ready`](Self::Ready)
    /// or [`Busy`](Self::Busy).
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Ready | Self::Busy { .. })
    }

    /// Returns `true` if a new task can be assigned to this plugin.
    ///
    /// Only plugins in the [`Ready`](Self::Ready) state accept work.
    pub fn can_acquire(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

impl fmt::Display for PluginLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starting => write!(f, "starting"),
            Self::Ready => write!(f, "ready"),
            Self::Busy { task_id } => write!(f, "busy (task {})", task_id),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed { reason } => write!(f, "failed: {}", reason),
        }
    }
}

/// A running plugin instance with its process handle, transport client, and
/// lifecycle metadata.
///
/// Created by the plugin manager when a plugin process is spawned. The
/// `grpc_client` field is only present for guest plugins compiled with the
/// `guest` feature.
pub struct PluginInstance {
    /// Static registry entry this instance was created from.
    pub entry: Arc<PluginEntry>,
    /// Current lifecycle state.
    pub lifecycle: PluginLifecycle,
    /// Handle to the spawned child process.
    ///
    /// Uses `tokio::process::Child` so the manager can `.await` process exit
    /// without blocking the async runtime. Set to `None` before spawn or after
    /// the process has been reaped.
    pub process: Option<tokio::process::Child>,
    /// gRPC client for communicating with guest plugins running inside a VM.
    ///
    /// `None` for host plugins or before the gRPC connection is established.
    pub grpc_client: Option<crate::transport::daemon::GrpcClient>,
    /// Wall-clock time when the plugin process was started.
    pub started_at: Option<Instant>,
    /// Wall-clock time of the most recent successful health check.
    pub last_health_check: Option<Instant>,
}

impl PluginInstance {
    /// Create a new instance in the [`Starting`](PluginLifecycle::Starting)
    /// state with no process or client attached.
    pub fn new(entry: Arc<PluginEntry>) -> Self {
        Self {
            entry,
            lifecycle: PluginLifecycle::Starting,
            process: None,
            grpc_client: None,
            started_at: None,
            last_health_check: None,
        }
    }
}

impl fmt::Debug for PluginInstance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginInstance")
            .field("plugin_id", &self.entry.id)
            .field("lifecycle", &self.lifecycle)
            .field("has_process", &self.process.is_some())
            .field("started_at", &self.started_at)
            .field("last_health_check", &self.last_health_check)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_initial_state_is_starting() {
        let state = PluginLifecycle::Starting;

        assert!(state.is_starting());
        assert!(!state.is_ready());
        assert!(!state.is_busy());
        assert!(!state.is_failed());
        assert!(!state.is_running());
        assert!(!state.can_acquire());
    }

    #[test]
    fn lifecycle_can_acquire_when_ready() {
        let state = PluginLifecycle::Ready;

        assert!(state.is_ready());
        assert!(state.is_running());
        assert!(state.can_acquire());
        assert!(!state.is_starting());
        assert!(!state.is_busy());
        assert!(!state.is_failed());
    }

    #[test]
    fn lifecycle_cannot_acquire_when_busy() {
        let state = PluginLifecycle::Busy { task_id: 42 };

        assert!(state.is_busy());
        assert!(state.is_running());
        assert!(!state.can_acquire());
        assert!(!state.is_ready());
        assert!(!state.is_starting());
        assert!(!state.is_failed());
    }

    #[test]
    fn lifecycle_cannot_acquire_when_failed() {
        let state = PluginLifecycle::Failed {
            reason: "segfault".into(),
        };

        assert!(state.is_failed());
        assert!(!state.can_acquire());
        assert!(!state.is_ready());
        assert!(!state.is_running());
        assert!(!state.is_starting());
        assert!(!state.is_busy());
    }

    #[test]
    fn lifecycle_is_running() {
        // Ready and Busy are running
        assert!(PluginLifecycle::Ready.is_running());
        assert!(PluginLifecycle::Busy { task_id: 1 }.is_running());

        // All other states are not running
        assert!(!PluginLifecycle::Starting.is_running());
        assert!(!PluginLifecycle::Stopping.is_running());
        assert!(!PluginLifecycle::Stopped.is_running());
        assert!(
            !PluginLifecycle::Failed {
                reason: "timeout".into()
            }
            .is_running()
        );
    }

    #[test]
    fn lifecycle_display() {
        assert_eq!(format!("{}", PluginLifecycle::Starting), "starting");
        assert_eq!(format!("{}", PluginLifecycle::Ready), "ready");
        assert_eq!(
            format!("{}", PluginLifecycle::Busy { task_id: 7 }),
            "busy (task 7)"
        );
        assert_eq!(format!("{}", PluginLifecycle::Stopping), "stopping");
        assert_eq!(format!("{}", PluginLifecycle::Stopped), "stopped");
        assert_eq!(
            format!(
                "{}",
                PluginLifecycle::Failed {
                    reason: "oom".into()
                }
            ),
            "failed: oom"
        );
    }

    #[test]
    fn lifecycle_stopping_and_stopped_cannot_acquire() {
        assert!(!PluginLifecycle::Stopping.can_acquire());
        assert!(!PluginLifecycle::Stopped.can_acquire());
    }
}
