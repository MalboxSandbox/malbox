pub mod artifact;
#[cfg(feature = "grpc")]
pub mod daemon;
pub mod error;
#[cfg(feature = "grpc")]
pub mod grpc;
#[cfg(feature = "ipc")]
pub mod ipc;
pub mod job_config;
pub mod messages;
#[cfg(feature = "grpc")]
pub mod plugin;
pub mod traits;

pub use traits::{TransportEmitter, TransportReceiver};

/// Well-known `result_name` for a plugin's structured report envelope.
///
/// Shared constant so the SDK (producer) and scheduler (consumer) agree
/// without either needing to depend on the other. The scheduler uses this
/// to tag matching `task_results` rows with `role = 'report'`.
pub const REPORT_RESULT_NAME: &str = "report";

/// Reserved `result_name` for a JSON object of task-wide shared-data updates.
pub const SHARED_RESULT_NAME: &str = "malbox.shared";

/// Reserved `result_name` (JSON `{}`) asking the daemon to skip this
/// artifact's later stages.
pub const DROP_RESULT_NAME: &str = "malbox.drop";
