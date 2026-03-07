//! Machine pooling for snapshot-capable providers
//!
//! This module implements a simple pool that manages pre-allocated machines.
//! The pool itself does NOT handle allocation or provisioning - that's the
//! responsibility of the MachineryManager. The pool just tracks machines
//! and their clean snapshots.

use crate::error::{ResourceError, Result};
use crate::{Allocate, Machine, MachineId, Snapshot, SnapshotId};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration for machine pool.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Minimum number of machines to keep ready.
    pub min_ready: usize,
    /// Maximum number of machines in the pool.
    pub max_size: usize,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            min_ready: 2,
            max_size: 10,
        }
    }
}

/// A machine in the pool with its clean snapshot.
struct PooledMachine {
    machine: Machine,
    clean_snapshot: SnapshotId,
    in_use: bool,
}

/// Machine pool for snapshot-capable providers.
///
/// This pool maintains a set of machines with their clean snapshots.
/// It does NOT handle allocation or provisioning - just pool management.
pub struct MachinePool {
    snapshot: Arc<dyn Snapshot>,
    allocate: Arc<dyn Allocate>,
    config: PoolConfig,
    machines: Arc<RwLock<HashMap<MachineId, PooledMachine>>>,
    available: Arc<RwLock<VecDeque<MachineId>>>,
}

impl MachinePool {
    /// Create a new empty machine pool.
    pub fn new(
        snapshot: Arc<dyn Snapshot>,
        allocate: Arc<dyn Allocate>,
        config: PoolConfig,
    ) -> Self {
        Self {
            snapshot,
            allocate,
            config,
            machines: Arc::new(RwLock::new(HashMap::new())),
            available: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Add a pre-allocated and provisioned machine to the pool.
    /// The caller is responsible for allocation and provisioning.
    pub async fn add_machine(&self, machine: Machine, clean_snapshot: SnapshotId) -> Result<MachineId> {
        let machine_id = machine.id.clone();

        info!("Adding machine {:?} to pool with clean snapshot", machine_id);

        let pooled = PooledMachine {
            machine,
            clean_snapshot,
            in_use: false,
        };

        let mut machines = self.machines.write().await;
        machines.insert(machine_id.clone(), pooled);

        let mut available = self.available.write().await;
        available.push_back(machine_id.clone());

        Ok(machine_id)
    }

    /// Check if pool can accept more machines.
    pub async fn can_add_more(&self) -> bool {
        let machines = self.machines.read().await;
        machines.len() < self.config.max_size
    }

    /// Get the number of machines needed to reach minimum ready count.
    pub async fn machines_needed(&self) -> usize {
        let total = self.machines.read().await.len();
        if total < self.config.min_ready {
            self.config.min_ready - total
        } else {
            0
        }
    }

    /// Acquire a machine from the pool.
    ///
    /// Removes the machine from the pool and returns ownership.
    /// The machine must be re-added via add_machine() if it's to be returned to the pool.
    pub async fn acquire(&self) -> Result<Machine> {
        let mut available = self.available.write().await;

        let machine_id = match available.pop_front() {
            Some(id) => id,
            None => {
                return Err(ResourceError::PoolExhausted);
            }
        };
        drop(available);

        let mut machines = self.machines.write().await;
        let mut pooled = machines
            .remove(&machine_id)
            .ok_or_else(|| ResourceError::MachineNotFound {
                id: format!("{:?}", machine_id),
            })?;

        // Restore to clean snapshot before returning
        debug!("Restoring machine {:?} to clean snapshot", machine_id);
        self.snapshot
            .restore_snapshot(&mut pooled.machine, &pooled.clean_snapshot)
            .await
            .map_err(|e| ResourceError::Provider(e.to_string()))?;

        // Return ownership of the machine to the caller
        Ok(pooled.machine)
    }

    /// Release a machine back to the pool.
    ///
    /// Note: This method expects the machine to already be in the pool (marked as in-use).
    /// If the machine was removed via acquire(), it must be re-added via add_machine() instead.
    pub async fn release(&self, machine_id: &MachineId) -> Result<()> {
        let machines = self.machines.read().await;

        // Check if machine exists in pool
        if !machines.contains_key(machine_id) {
            // Machine was removed from pool (via acquire), cannot release
            // This is expected behavior - just log and return Ok
            debug!(
                "Machine {:?} not in pool (was removed via acquire)",
                machine_id
            );
            return Ok(());
        }
        drop(machines);

        // Machine is in pool, mark as available
        let mut available = self.available.write().await;
        available.push_back(machine_id.clone());

        debug!("Machine {:?} released back to pool", machine_id);

        Ok(())
    }

    /// Shutdown the pool and deallocate all machines.
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down machine pool");

        let mut machines = self.machines.write().await;
        let machine_ids: Vec<_> = machines.keys().cloned().collect();

        for machine_id in machine_ids {
            if let Some(pooled) = machines.remove(&machine_id) {
                debug!("Deallocating machine {:?}", machine_id);
                self.allocate
                    .deallocate(&pooled.machine)
                    .await
                    .map_err(|e| {
                        warn!("Failed to deallocate machine {:?}: {}", machine_id, e);
                        ResourceError::DeallocationFailed(e.to_string())
                    })?;
            }
        }

        Ok(())
    }

    /// Get pool statistics.
    pub async fn stats(&self) -> PoolStats {
        let machines = self.machines.read().await;
        let available = self.available.read().await;

        PoolStats {
            total: machines.len(),
            available: available.len(),
            in_use: machines.values().filter(|m| m.in_use).count(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total: usize,
    pub available: usize,
    pub in_use: usize,
}
