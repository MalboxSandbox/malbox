use serde::{Deserialize, Serialize};

/// Which transport mechanism to use for guest communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum TransportKind {
    /// Built-in gRPC transport over the network.
    Grpc,
    /// Provider-native transport (e.g., "virtio-serial", "qemu-guest-agent").
    Provider(String),
}

impl From<String> for TransportKind {
    fn from(s: String) -> Self {
        if s == "grpc" {
            TransportKind::Grpc
        } else {
            TransportKind::Provider(s)
        }
    }
}

impl From<TransportKind> for String {
    fn from(kind: TransportKind) -> Self {
        match kind {
            TransportKind::Grpc => "grpc".to_string(),
            TransportKind::Provider(name) => name,
        }
    }
}

/// Guest access configuration.
///
/// Configures how the daemon communicates with guest VMs for file transfer
/// and command execution. When absent, guest access is disabled and tasks
/// requiring it will fail.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestAccessConfig {
    pub transport: TransportKind,

    /// Transport-specific configuration passed through to the transport.
    #[serde(default = "default_transport_config")]
    pub config: toml::Value,
}

fn default_transport_config() -> toml::Value {
    toml::Value::Table(toml::map::Map::new())
}
