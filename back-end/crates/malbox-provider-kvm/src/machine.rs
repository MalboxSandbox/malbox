use async_trait::async_trait;
use malbox_machinery::machine::{Machine, MachineId, MachineSpec, MachineState};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::info;
use virt::{connect::Connect, domain::Domain};

use crate::error::KvmError;

/// KVM machine handle.
pub struct KvmMachine {
    pub id: MachineId,
    pub domain_name: String,
    pub spec: MachineSpec,
    pub state: MachineState,
    // NOTE:
    // We have `conn` field here and in provider struct.
    // That might seem redundant, maybe it is worth to check for other ways.
    // One way would be to make Machine dependent on Provider. But I am not sure
    // if this is what we want- maybe the current approach is just fine.
    pub conn: Arc<Connect>,
}

#[async_trait]
impl Machine for KvmMachine {
    type Error = KvmError;

    fn id(&self) -> &MachineId {
        &self.id
    }

    fn spec(&self) -> &MachineSpec {
        &self.spec
    }

    fn state(&self) -> MachineState {
        self.state.clone()
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &self.domain_name)
            .map_err(|e| KvmError::Libvirt(format!("Domain not found: {}", e)))?;

        if !domain
            .is_active()
            .map_err(|e| KvmError::Libvirt(e.to_string()))?
        {
            domain
                .create()
                .map_err(|e| KvmError::Libvirt(format!("Failed to start: {}", e)))?;
        }

        self.state = MachineState::Running;

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &self.domain_name)
            .map_err(|e| KvmError::Libvirt(format!("Domain not found: {}", e)))?;

        if domain
            .is_active()
            .map_err(|e| KvmError::Libvirt(e.to_string()))?
        {
            // TODO: better code for graceful shutdowns..
            // This is supposed to be an attempt for a graceful shutdown
            if domain.shutdown().is_ok() {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            // Force stop if still running after graceful shutdown

            if domain.is_active().unwrap_or(false) {
                domain
                    .destroy()
                    .map_err(|e| KvmError::Libvirt(format!("Failed to stop: {}", e)))?;
            }
        }

        self.state = MachineState::Stopped;

        Ok(())
    }

    async fn reboot(&mut self) -> Result<(), Self::Error> {
        todo!()
    }

    async fn wait_ready(&mut self, timeout: Duration) -> Result<(), Self::Error> {
        info!("Waiting for machine {} to be ready", self.domain_name);

        todo!()
    }
}
