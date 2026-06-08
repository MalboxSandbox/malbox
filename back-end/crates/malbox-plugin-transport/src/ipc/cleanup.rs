//! Reclaim stale iceoryx2 resources from a previous unclean daemon exit, so a
//! plugin's `open_or_create` doesn't fail with `ServiceInCorruptedState`.

use crate::error::{Result, TransportError};

use super::IpcService;
use iceoryx2::prelude::*;

/// Outcome of a [`cleanup_stale_resources`] pass.
#[derive(Debug, Default, Clone, Copy)]
pub struct CleanupReport {
    /// Dead nodes whose stale resources were successfully removed.
    pub reclaimed: usize,
    /// Dead nodes that could not be cleaned (e.g. a concurrent cleanup raced
    /// us, or a version mismatch). Non-fatal; left for the next pass.
    pub failed: usize,
}

impl CleanupReport {
    /// Whether the pass touched anything worth reporting.
    pub fn is_empty(&self) -> bool {
        self.reclaimed == 0 && self.failed == 0
    }
}

/// Remove the stale resources of every dead iceoryx2 node, returning the
/// counts. Run once at startup, before any node or service is created.
pub fn cleanup_stale_resources() -> Result<CleanupReport> {
    let mut report = CleanupReport::default();

    Node::<IpcService>::list(Config::global_config(), |node_state| {
        if let NodeState::Dead(view) = node_state {
            match view.try_remove_stale_resources() {
                Ok(()) => report.reclaimed += 1,
                Err(_) => report.failed += 1,
            }
        }
        CallbackProgression::Continue
    })
    .map_err(|e| TransportError::Ipc(Box::new(e)))?;

    Ok(report)
}
