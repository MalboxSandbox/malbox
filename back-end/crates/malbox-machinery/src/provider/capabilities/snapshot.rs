//! Snapshot capability module.

use crate::machine::Machine;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

// TODO: replace String with proper UUIDv4
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

/// Snapshot capability: Save and restore machine state.
///
/// Providers that implement this capability can save a machine's state
/// and restore it later. This is essential for pooled strategies that
/// need to quickly reset machines between tasks.
#[async_trait]
pub trait Snapshot: Send + Sync {
    /// Create a snapshot of a machine's current state.
    ///
    /// This should save the machine's disk, memory, and configuration state.
    /// The machine can continue running after snapshot creation.
    ///
    /// # Arguments
    /// * `machine` - The machine to snapshot
    /// * `name` - Human-readable name for the snapshot
    ///
    /// # Returns
    /// * `Ok(SnapshotId)` - ID of the created snapshot
    /// * `Err(_)` - If snapshot creation fails
    async fn create_snapshot(
        &self,
        machine: &Machine,
        name: &str,
    ) -> Result<SnapshotId, Box<dyn Error + Send + Sync>>;

    /// Restore a machine to a previous snapshot.
    ///
    /// This should revert the machine to the exact state it was in when
    /// the snapshot was created. The machine will typically be stopped
    /// and restarted during this process.
    ///
    /// # Arguments
    /// * `machine` - The machine to restore
    /// * `snapshot` - The snapshot ID to restore to
    ///
    /// # Returns
    /// * `Ok(())` - Machine successfully restored
    /// * `Err(_)` - If restoration fails
    async fn restore_snapshot(
        &self,
        machine: &Machine,
        snapshot: &SnapshotId,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// Delete a snapshot.
    ///
    /// This should remove the snapshot and free associated storage.
    ///
    /// # Arguments
    /// * `snapshot` - The snapshot ID to delete
    ///
    /// # Returns
    /// * `Ok(())` - Snapshot successfully deleted
    /// * `Err(_)` - If deletion fails
    async fn delete_snapshot(
        &self,
        snapshot: &SnapshotId,
    ) -> Result<(), Box<dyn Error + Send + Sync>>;

    /// List all snapshots for a given machine.
    ///
    /// # Arguments
    /// * `machine` - The machine to list snapshots for
    ///
    /// # Returns
    /// * `Ok(Vec<SnapshotInfo>)` - List of snapshots
    /// * `Err(_)` - If listing fails
    async fn list_snapshots(
        &self,
        machine: &Machine,
    ) -> Result<Vec<SnapshotInfo>, Box<dyn Error + Send + Sync>>;
}
