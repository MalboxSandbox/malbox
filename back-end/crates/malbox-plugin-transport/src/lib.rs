pub mod daemon;
pub mod error;
pub mod grpc;
#[cfg(feature = "ipc")]
pub mod ipc;
pub mod messages;
pub mod plugin;
pub mod traits;

pub use traits::{TransportEmitter, TransportReceiver};

/// Well-known `result_name` for a plugin's structured report envelope.
///
/// Shared constant so the SDK (producer) and scheduler (consumer) agree
/// without either needing to depend on the other. The scheduler uses this
/// to tag matching `task_results` rows with `role = 'report'`.
pub const REPORT_RESULT_NAME: &str = "report";
