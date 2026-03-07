use serde::{Deserialize, Serialize};

/// Guest access configuration.
///
/// Configures how the daemon communicates with guest VMs for file transfer
/// and command execution. When absent, guest access is disabled and tasks
/// requiring it will fail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestAccessConfig {
    /// Name of the transport to use (e.g., "virtio-serial", "grpc").
    ///
    /// This is resolved at startup: first checked against the provider's
    /// native transports, then against registered standalone transports.
    pub transport: String,

    /// Transport-specific configuration passed through to the transport.
    #[serde(default = "default_transport_config")]
    pub config: toml::Value,
}

fn default_transport_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}
