//! Transport resolution for guest access.
//!
//! Resolves the user-configured transport name to either a provider-native
//! transport or the gRPC guest access client at daemon startup.

use crate::error::{ResourceError, Result};
use malbox_config::{GuestAccessConfig, TransportKind};
use malbox_machinery::{GuestAccess, ProviderHandle};
use std::sync::Arc;

/// A resolved transport configuration.
///
/// Created at daemon startup by checking the user's configured transport
/// against the provider's native capabilities, or falling back to gRPC.
#[non_exhaustive]
pub enum ResolvedTransport {
    /// Provider handles this transport natively (e.g., virtio-serial in libvirt).
    Native {
        /// Provider capability that opens guest sessions.
        guest_access: Arc<dyn GuestAccess>,
        /// Configured transport name passed to the provider.
        transport_name: String,
        /// Transport-specific configuration table.
        config: toml::Value,
    },
    /// gRPC transport — connects to a guest plugin over the network.
    Grpc {
        /// Endpoint URI of the guest plugin's gRPC server.
        address: String,
    },
}

impl std::fmt::Debug for ResolvedTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Native {
                transport_name,
                config,
                ..
            } => f
                .debug_struct("Native")
                .field("transport_name", transport_name)
                .field("config", config)
                .finish_non_exhaustive(),
            Self::Grpc { address } => f.debug_struct("Grpc").field("address", address).finish(),
        }
    }
}

/// Whether machines can reach the host over the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NetworkMode {
    /// Machines have host connectivity; gRPC transport is usable.
    Connected,
    /// Machines are on an isolated network; gRPC transport cannot work.
    Isolated,
}

/// Resolve the configured transport against the provider and gRPC.
///
/// Called once at daemon startup. `network` indicates whether machines are
/// on an isolated network (no host connectivity), which prevents gRPC from
/// working.
///
/// # Errors
///
/// Fails if the provider does not offer the configured native transport, or
/// gRPC is requested while `network` is [`NetworkMode::Isolated`].
pub fn resolve_transport(
    provider: &ProviderHandle,
    guest_config: &GuestAccessConfig,
    network: NetworkMode,
) -> Result<ResolvedTransport> {
    match &guest_config.transport {
        TransportKind::Provider(name) => {
            let guest_access =
                provider
                    .guest_access()
                    .ok_or_else(|| ResourceError::UnknownTransport {
                        name: name.clone(),
                        provider: provider.name().to_string(),
                        available: vec!["grpc".to_string()],
                    })?;

            let supported = guest_access.supported_transports();
            if !supported.contains(&name.as_str()) {
                let mut available: Vec<String> =
                    supported.iter().map(ToString::to_string).collect();
                available.push("grpc".to_string());
                return Err(ResourceError::UnknownTransport {
                    name: name.clone(),
                    provider: provider.name().to_string(),
                    available,
                });
            }

            Ok(ResolvedTransport::Native {
                guest_access,
                transport_name: name.clone(),
                config: guest_config.config.clone(),
            })
        }
        TransportKind::Grpc => {
            if network == NetworkMode::Isolated {
                return Err(ResourceError::IncompatibleTransport {
                    transport: "grpc".to_string(),
                    reason: "gRPC requires network connectivity but network mode is isolated"
                        .to_string(),
                });
            }

            let address = guest_config
                .config
                .get("address")
                .and_then(|v| v.as_str())
                .unwrap_or("http://[::1]:50051")
                .to_string();

            Ok(ResolvedTransport::Grpc { address })
        }
        other => Err(ResourceError::UnknownTransport {
            name: String::from(other.clone()),
            provider: provider.name().to_string(),
            available: vec!["grpc".to_string()],
        }),
    }
}
