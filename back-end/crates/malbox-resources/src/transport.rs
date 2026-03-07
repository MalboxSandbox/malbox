//! Transport resolution for guest access.
//!
//! Resolves the user-configured transport name to either a provider-native
//! transport or the gRPC guest access client at daemon startup.

use crate::error::{ResourceError, Result};
use malbox_config::GuestAccessConfig;
use malbox_machinery::machine::Machine;
use malbox_machinery::{ExecOptions, ExecResult, GuestSession, GuestStatus};
use malbox_machinery::{GuestAccess, NetworkMode, ProviderHandle};
use malbox_plugin_transport::daemon::GrpcClient;
use std::sync::Arc;

/// A resolved transport ready to open guest sessions.
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
    Grpc {
        address: String,
        config: toml::Value,
    },
}

impl ResolvedTransport {
    /// Open a guest session to the given machine.
    ///
    /// For native transports, delegates to the provider. For gRPC, connects
    /// to the guest plugin server and wraps the client in a `GuestSession`.
    pub async fn open_session(
        &self,
        machine: &Machine,
    ) -> std::result::Result<Box<dyn GuestSession>, Box<dyn std::error::Error + Send + Sync>> {
        match self {
            ResolvedTransport::Native {
                guest_access,
                transport_name,
                config,
            } => {
                guest_access
                    .open_session(transport_name, machine, config)
                    .await
            }
            ResolvedTransport::Grpc { address, .. } => {
                let client = GrpcClient::connect(address).await?;
                Ok(Box::new(GrpcGuestSession {
                    client: tokio::sync::Mutex::new(client),
                }))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GrpcGuestSession adapter
// ---------------------------------------------------------------------------

/// Adapts `GrpcClient` (from `malbox-plugin-internal`) to `GuestSession`
/// (from `malbox-machinery`).
///
/// This adapter lives here in `malbox-resources` because it bridges two
/// independent crates that must not depend on each other.
struct GrpcGuestSession {
    client: tokio::sync::Mutex<GrpcClient>,
}

#[async_trait::async_trait]
impl GuestSession for GrpcGuestSession {
    async fn push_file(
        &self,
        source: &std::path::Path,
        dest: &str,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = tokio::fs::read(source).await?;
        let mut client = self.client.lock().await;
        let resp = client.push_file(dest, data).await?;
        if !resp.success {
            return Err(resp.error_message.into());
        }
        Ok(())
    }

    async fn pull_file(
        &self,
        source: &str,
        dest: &std::path::Path,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut client = self.client.lock().await;
        let data = client.pull_file(source).await?;
        tokio::fs::write(dest, &data).await?;
        Ok(())
    }

    async fn execute(
        &self,
        command: &str,
        args: &[String],
        opts: ExecOptions,
    ) -> std::result::Result<ExecResult, Box<dyn std::error::Error + Send + Sync>> {
        let mut client = self.client.lock().await;
        let resp = client
            .execute_command(
                command,
                args,
                opts.cwd.as_deref(),
                &opts.env,
                opts.timeout.map(|d| d.as_millis() as u64),
                opts.background,
            )
            .await?;
        Ok(ExecResult {
            exit_code: resp.exit_code,
            stdout: resp.stdout,
            stderr: resp.stderr,
        })
    }

    async fn health_check(
        &self,
    ) -> std::result::Result<GuestStatus, Box<dyn std::error::Error + Send + Sync>> {
        let mut client = self.client.lock().await;
        let resp = client.health_check().await?;
        if resp.ready {
            Ok(GuestStatus::Ready)
        } else {
            Ok(GuestStatus::NotReady {
                reason: resp.reason,
            })
        }
    }

    async fn close(
        self: Box<Self>,
    ) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Transport resolution
// ---------------------------------------------------------------------------

/// Resolve the configured transport against the provider and gRPC.
///
/// Called once at daemon startup. Returns a `ResolvedTransport` that can
/// open sessions at task execution time.
pub fn resolve_transport(
    provider: &ProviderHandle,
    guest_config: &GuestAccessConfig,
    default_network_mode: &NetworkMode,
) -> Result<ResolvedTransport> {
    let name = &guest_config.transport;

    // 1. Check provider-native transports
    if let Some(guest_access) = provider.guest_access() {
        let supported = guest_access.supported_transports();
        if supported.iter().any(|s| *s == name.as_str()) {
            return Ok(ResolvedTransport::Native {
                guest_access,
                transport_name: name.clone(),
                config: guest_config.config.clone(),
            });
        }
    }

    // 2. gRPC transport
    if name == "grpc" {
        if matches!(default_network_mode, NetworkMode::Isolated { .. }) {
            return Err(ResourceError::IncompatibleTransport {
                transport: name.clone(),
                reason: "gRPC requires network connectivity but network mode is Isolated"
                    .to_string(),
            });
        }

        let address = guest_config
            .config
            .get("address")
            .and_then(|v| v.as_str())
            .unwrap_or("http://[::1]:50051")
            .to_string();

        return Ok(ResolvedTransport::Grpc {
            address,
            config: guest_config.config.clone(),
        });
    }

    // 3. Not found
    let mut available: Vec<String> = provider
        .guest_access()
        .map(|ga| {
            ga.supported_transports()
                .iter()
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default();
    available.push("grpc".to_string());

    Err(ResourceError::UnknownTransport {
        name: name.clone(),
        provider: provider.name().to_string(),
        available,
    })
}
