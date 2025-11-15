use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::Provider;

/// Marker trait that tags provider capabilities.
pub trait ProviderExtension: Provider {}

// TODO: Proper type.. same with MachineId
/// Unique identifier for a snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(pub String);

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata about a snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotInfo {
    /// Snapshot identifier.
    pub id: SnapshotId,
    /// Human-readable name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Creation timestamp (Unix epoch seconds).
    pub created_at: i64,
    /// Size in bytes (if available).
    pub size_bytes: Option<u64>,
    /// Whether this is the current active snapshot.
    pub is_current: bool,
}

/// Snapshot extension trait.
///
/// Provides snapshot capabilities for VM state management.
#[async_trait]
pub trait Snapshots: Provider + ProviderExtension {
    /// Create a new snapshot of the machine's current state.
    async fn create_snapshot(
        &self,
        machine: &Self::Machine,
        name: &str,
    ) -> Result<SnapshotId, Self::Error>;

    /// Restore a machine to a previous snapshot state.
    async fn restore_snapshot(
        &self,
        machine: &mut Self::Machine,
        snapshot: &SnapshotId,
    ) -> Result<(), Self::Error>;

    /// Delete a snapshot.
    async fn delete_snapshot(&self, snapshot: &SnapshotId) -> Result<(), Self::Error>;

    /// List all snapshots for a given machine.
    async fn list_snapshots(
        &self,
        machine: &Self::Machine,
    ) -> Result<Vec<SnapshotInfo>, Self::Error>;
}
