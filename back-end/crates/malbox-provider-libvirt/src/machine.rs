use async_trait::async_trait;
use malbox_machinery::machine::{Machine, MachineEndpoint, MachineId, MachineSpec, MachineState};
use std::{
    any::Any,
    net::IpAddr,
    sync::Arc,
    time::{Duration, Instant},
};
use tracing::info;
use virt::{connect::Connect, domain::Domain};

use crate::error::LibvirtError;

/// KVM machine handle.
pub struct LibvirtMachine {
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
impl Machine for LibvirtMachine {
    type Error = LibvirtError;

    fn id(&self) -> &MachineId {
        &self.id
    }

    fn spec(&self) -> &MachineSpec {
        &self.spec
    }

    fn state(&self) -> MachineState {
        self.state
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &self.domain_name)?;

        if !domain.is_active()? {
            domain.create()?;
        }

        self.state = MachineState::Running;

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &self.domain_name)?;

        if domain.is_active()? {
            // TODO: better code for graceful shutdowns..
            // This is supposed to be an attempt for a graceful shutdown
            if domain.shutdown().is_ok() {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }

            // Force stop if still running after graceful shutdown
            if domain.is_active().unwrap_or(false) {
                domain.destroy()?;
            }
        }

        self.state = MachineState::Stopped;

        Ok(())
    }

    async fn reboot(&mut self) -> Result<(), Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &self.domain_name)?;

        if !domain.is_active()? {
            return Err(LibvirtError::Libvirt(
                "Cannot reboot a domain that is not running".to_string(),
            ));
        }

        // Flags: 0 means default reboot behavior (attempt graceful reboot via guest agent or ACPI)
        domain.reboot(0)?;

        self.state = MachineState::Starting;

        Ok(())
    }

    /// Check if a machine is ready for analysis.
    async fn wait_ready(&self, timeout: Duration) -> Result<(), Self::Error> {
        let domain = Domain::lookup_by_name(&self.conn, &self.domain_name)?;

        info!(
            "Domain {} - waiting for machine to be ready",
            self.domain_name
        );

        let start = Instant::now();

        while start.elapsed() < timeout {
            // Check if domain is still running
            if !domain.is_active()? {
                return Err(LibvirtError::Libvirt(
                    "Domain stopped while waiting for readiness".to_string(),
                ));
            }

            // Try to get guest agent status via interface addresses
            // If guest agent is working, this will succeed
            let agent_ready = domain
                .interface_addresses(
                    virt::sys::VIR_DOMAIN_INTERFACE_ADDRESSES_SRC_AGENT as u32,
                    0,
                )
                .is_ok();

            if agent_ready {
                info!(
                    "Domain {} is ready (guest agent responding)",
                    self.domain_name
                );
                return Ok(());
            }

            // If guest agent isn't available yet, keep waiting
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        Err(LibvirtError::Timeout {
            operation: "wait_ready".to_string(),
            duration: timeout,
        })
    }

    async fn wait_network(&self, timeout: Duration) -> Result<(), Self::Error> {
        let start = Instant::now();

        while start.elapsed() < timeout {
            if self.endpoint().is_some() {
                info!("Domain {} has network connectivity", self.domain_name);
                return Ok(());
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        Err(LibvirtError::Timeout {
            operation: "wait_network".to_string(),
            duration: timeout,
        })
    }

    fn endpoint(&self) -> Option<MachineEndpoint> {
        if !matches!(self.state, MachineState::Running) {
            return None;
        }

        let ip = self.get_ip_address()?;

        Some(MachineEndpoint {
            address: ip,
            id: self.id.0.clone(),
            platform: self.spec.platform.clone(),
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl LibvirtMachine {
    /// Get IP address from libvirt (guest agent)
    fn get_ip_address(&self) -> Option<IpAddr> {
        let domain = Domain::lookup_by_name(&self.conn, &self.domain_name).ok()?;

        // Try guest agent (most reliable if agent is installed)
        if let Ok(interfaces) = domain.interface_addresses(
            virt::sys::VIR_DOMAIN_INTERFACE_ADDRESSES_SRC_AGENT as u32,
            0,
        ) {
            for iface in interfaces {
                for addr in iface.addrs {
                    if let Ok(ip) = addr.addr.parse::<IpAddr>() {
                        if !ip.is_loopback() {
                            return Some(ip);
                        }
                    }
                }
            }
        }

        // TODO: Add DHCP lease lookup method
        None
    }

    /// Get network name from spec.
    fn get_network_name(&self) -> Result<String, LibvirtError> {
        use malbox_machinery::machine::NetworkMode;

        match &self.spec.network.mode {
            NetworkMode::Nat => Ok("default".to_string()),
            NetworkMode::Isolated { subnet } => {
                Ok(format!("malbox-isolated-{}", subnet.replace('/', "-")))
            }
            NetworkMode::Bridged { interface } => Ok(interface.clone()),
        }
    }
}
