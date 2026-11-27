//! Machine bridge network and static IP assignment.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::net::Ipv4Addr;

use bollard::Docker;
use bollard::models::{Ipam, IpamConfig, NetworkCreateRequest};
use tracing::info;

use crate::error::status;
use crate::{ContainerError, ContainerProviderConfig, LABEL_MANAGED};

/// IPv4 network in CIDR form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Ipv4Subnet {
    network: Ipv4Addr,
    prefix: u8,
}

impl Ipv4Subnet {
    /// Parse `a.b.c.d/p` with `p` in `8..=29` and no host bits set.
    pub(crate) fn parse(s: &str) -> Result<Self, ContainerError> {
        let invalid = || {
            ContainerError::Config(format!(
                "invalid subnet '{s}': expected an IPv4 network address with prefix 8..=29"
            ))
        };
        let (addr, prefix) = s.split_once('/').ok_or_else(invalid)?;
        let addr: Ipv4Addr = addr.parse().map_err(|_| invalid())?;
        let prefix: u8 = prefix.parse().map_err(|_| invalid())?;
        if !(8..=29).contains(&prefix) {
            return Err(invalid());
        }
        let subnet = Self {
            network: addr,
            prefix,
        };
        if u32::from(addr) & !subnet.mask() != 0 {
            return Err(invalid());
        }
        Ok(subnet)
    }

    fn mask(&self) -> u32 {
        u32::MAX << (32 - self.prefix)
    }

    fn broadcast(&self) -> u32 {
        u32::from(self.network) | !self.mask()
    }

    /// First host address, used as the bridge gateway.
    pub(crate) fn gateway(&self) -> Ipv4Addr {
        Ipv4Addr::from(u32::from(self.network) + 1)
    }

    /// Assignable machine addresses: after the gateway, before broadcast.
    pub(crate) fn hosts(&self) -> impl Iterator<Item = Ipv4Addr> {
        (u32::from(self.network) + 2..self.broadcast()).map(Ipv4Addr::from)
    }
}

impl fmt::Display for Ipv4Subnet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.network, self.prefix)
    }
}

/// Lowest host address of `subnet` not in `taken`.
pub(crate) fn pick_ip(subnet: &Ipv4Subnet, taken: &HashSet<Ipv4Addr>) -> Option<Ipv4Addr> {
    subnet.hosts().find(|ip| !taken.contains(ip))
}

/// Create the machine network if missing; verify its subnet otherwise.
pub(crate) async fn ensure(
    docker: &Docker,
    cfg: &ContainerProviderConfig,
    subnet: &Ipv4Subnet,
) -> Result<(), ContainerError> {
    let expected = subnet.to_string();
    match docker.inspect_network(&cfg.network, None).await {
        Ok(network) => {
            let configs = network
                .ipam
                .and_then(|ipam| ipam.config)
                .unwrap_or_default();
            if configs
                .iter()
                .any(|c| c.subnet.as_deref() == Some(expected.as_str()))
            {
                Ok(())
            } else {
                Err(ContainerError::Config(format!(
                    "network '{}' exists with a different subnet; expected {expected}",
                    cfg.network
                )))
            }
        }
        Err(e) if status(&e) == Some(404) => {
            docker
                .create_network(NetworkCreateRequest {
                    name: cfg.network.clone(),
                    driver: Some("bridge".to_string()),
                    internal: Some(cfg.internal),
                    ipam: Some(Ipam {
                        config: Some(vec![IpamConfig {
                            subnet: Some(expected.clone()),
                            gateway: Some(subnet.gateway().to_string()),
                            ..Default::default()
                        }]),
                        ..Default::default()
                    }),
                    labels: Some(HashMap::from([(
                        LABEL_MANAGED.to_string(),
                        "true".to_string(),
                    )])),
                    ..Default::default()
                })
                .await?;
            info!(network = %cfg.network, subnet = %expected, internal = cfg.internal, "Created container network");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    #[test]
    fn parse_and_gateway() {
        let subnet = Ipv4Subnet::parse("172.30.0.0/24").unwrap();
        assert_eq!(subnet.gateway(), ip("172.30.0.1"));
        assert_eq!(subnet.to_string(), "172.30.0.0/24");
    }

    #[test]
    fn pick_ip_takes_lowest_free_host() {
        let subnet = Ipv4Subnet::parse("172.30.0.0/24").unwrap();
        assert_eq!(pick_ip(&subnet, &HashSet::new()), Some(ip("172.30.0.2")));
        let taken = HashSet::from([ip("172.30.0.2"), ip("172.30.0.3")]);
        assert_eq!(pick_ip(&subnet, &taken), Some(ip("172.30.0.4")));
    }

    #[test]
    fn pick_ip_exhausted() {
        // /30 is rejected by `parse`; build it directly to exercise exhaustion.
        let subnet = Ipv4Subnet {
            network: ip("10.0.0.0"),
            prefix: 30,
        };
        assert_eq!(pick_ip(&subnet, &HashSet::from([ip("10.0.0.2")])), None);
    }

    #[test]
    fn parse_rejects_invalid() {
        assert!(Ipv4Subnet::parse("::/64").is_err());
        assert!(Ipv4Subnet::parse("10.0.0.0/31").is_err());
        assert!(Ipv4Subnet::parse("10.0.0.5/24").is_err());
    }
}
