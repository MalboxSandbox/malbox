//! Machine pooling for snapshot-capable providers
//!
//! This module implements a DB-backed pool that manages pre-allocated machines.
//! The pool keeps an in-memory cache (Vec<PooledMachine> + VecDeque<MachineId>)
//! for fast acquisition, with the database as the source of truth for persistence
//! across restarts.

use crate::error::{ResourceError, Result};
use crate::{Allocate, Machine, MachineId, Snapshot, SnapshotId};
use malbox_database::PgPool;
use std::collections::VecDeque;
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

/// A machine in the pool with its clean snapshot and DB row ID.
pub struct PooledMachine {
    pub machine: Machine,
    pub clean_snapshot: SnapshotId,
    pub db_id: i32,
}

/// Machine pool for snapshot-capable providers.
///
/// Maintains a set of machines with their clean snapshots, backed by a
/// database for persistence. The in-memory cache provides fast acquisition
/// while the DB is the source of truth for reconciliation on restart.
pub struct MachinePool {
    snapshot: Arc<dyn Snapshot>,
    allocate: Arc<dyn Allocate>,
    config: PoolConfig,
    db: PgPool,
    machines: Arc<RwLock<Vec<PooledMachine>>>,
    available: Arc<RwLock<VecDeque<MachineId>>>,
}

impl MachinePool {
    /// Create a new empty machine pool.
    pub fn new(
        snapshot: Arc<dyn Snapshot>,
        allocate: Arc<dyn Allocate>,
        config: PoolConfig,
        db: PgPool,
    ) -> Self {
        Self {
            snapshot,
            allocate,
            config,
            db,
            machines: Arc::new(RwLock::new(Vec::new())),
            available: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Load existing pool machines from the database for reconciliation.
    pub async fn load_from_db(
        &self,
    ) -> Result<Vec<malbox_database::repositories::machinery::Machine>> {
        malbox_database::repositories::machinery::fetch_pool_machines(&self.db)
            .await
            .map_err(|e| ResourceError::Database(e.to_string()))
    }

    /// Add a pre-allocated and provisioned machine to the pool.
    /// The caller is responsible for allocation and provisioning.
    pub async fn add_machine(
        &self,
        machine: Machine,
        clean_snapshot: SnapshotId,
        db_id: i32,
    ) -> Result<MachineId> {
        let machine_id = machine.id.clone();

        info!(
            "Adding machine {:?} to pool with clean snapshot (db_id={})",
            machine_id, db_id
        );

        let pooled = PooledMachine {
            machine,
            clean_snapshot,
            db_id,
        };

        let mut machines = self.machines.write().await;
        machines.push(pooled);

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
    /// Restores the machine to its clean snapshot and removes it from
    /// the in-memory cache. The machine must be re-added via add_machine()
    /// if it's to be returned to the pool.
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
        let idx = machines
            .iter()
            .position(|p| p.machine.id == machine_id)
            .ok_or_else(|| ResourceError::MachineNotFound {
                id: format!("{:?}", machine_id),
            })?;

        let pooled = machines.remove(idx);

        // Restore to clean snapshot before returning
        debug!("Restoring machine {:?} to clean snapshot", machine_id);
        self.snapshot
            .restore_snapshot(&pooled.machine, &pooled.clean_snapshot)
            .await
            .map_err(|e| ResourceError::Provider(e.to_string()))?;

        // Return ownership of the machine to the caller
        Ok(pooled.machine)
    }

    /// Release a machine back to the pool.
    ///
    /// Pushes the machine ID back onto the available queue. The machine
    /// must still exist in the machines Vec (i.e., it was not fully removed).
    pub async fn release(&self, machine_id: &MachineId) -> Result<()> {
        let machines = self.machines.read().await;

        // Check if machine exists in pool
        let exists = machines.iter().any(|p| &p.machine.id == machine_id);
        if !exists {
            debug!(
                "Machine {:?} not in pool (was removed via acquire)",
                machine_id
            );
            return Ok(());
        }
        drop(machines);

        let mut available = self.available.write().await;
        available.push_back(machine_id.clone());

        debug!("Machine {:?} released back to pool", machine_id);

        Ok(())
    }

    /// Shutdown the pool and deallocate all machines.
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down machine pool");

        let mut machines = self.machines.write().await;

        while let Some(pooled) = machines.pop() {
            debug!("Deallocating machine {:?}", pooled.machine.id);
            self.allocate
                .deallocate(&pooled.machine)
                .await
                .map_err(|e| {
                    warn!(
                        "Failed to deallocate machine {:?}: {}",
                        pooled.machine.id, e
                    );
                    ResourceError::DeallocationFailed(e.to_string())
                })?;
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
            in_use: machines.len().saturating_sub(available.len()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolStats {
    pub total: usize,
    pub available: usize,
    pub in_use: usize,
}
