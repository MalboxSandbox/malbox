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
pub enum ResolvedTransport {
    /// Provider handles this transport natively (e.g., virtio-serial in libvirt).
    Native {
        guest_access: Arc<dyn GuestAccess>,
        transport_name: String,
        config: toml::Value,
    },
    /// gRPC transport — connects to a guest plugin over the network.
    Grpc { address: String },
}

/// Resolve the configured transport against the provider and gRPC.
///
/// Called once at daemon startup. The `network_isolated` flag indicates
/// whether machines are on an isolated network (no host connectivity),
/// which prevents gRPC from working.
pub fn resolve_transport(
    provider: &ProviderHandle,
    guest_config: &GuestAccessConfig,
    network_isolated: bool,
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
                let mut available: Vec<String> = supported.iter().map(|s| s.to_string()).collect();
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
            if network_isolated {
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
    }
}
