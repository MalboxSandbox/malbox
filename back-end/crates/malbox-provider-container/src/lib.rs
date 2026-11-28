//! Docker/Podman container provider for malbox.
//!
//! Runs Linux machines as containers through the Docker Engine API (Podman's
//! compat API works too). Snapshots are committed images, so a revert is a
//! container recreate that takes well under a second.
//!
//! Host requirements: rootful Docker or rootful Podman (rootless container
//! IPs are not reachable from the host). Only Linux machines are supported.

use std::collections::HashMap;

use bollard::Docker;
use malbox_machinery::MachineId;
use malbox_machinery::provider::TomlValue;
use malbox_machinery_macros::RegisterProvider;

mod capabilities;
mod error;
mod image;
mod network;
mod session;
mod spec;

pub use error::ContainerError;

use network::Ipv4Subnet;

/// Label marking containers and networks managed by malbox (value `"true"`).
pub(crate) const LABEL_MANAGED: &str = "malbox.managed";
/// Label carrying the machine id.
pub(crate) const LABEL_MACHINE: &str = "malbox.machine";
/// Label carrying the JSON [`spec::ContainerSpec`].
pub(crate) const LABEL_SPEC: &str = "malbox.spec";
/// Label carrying a snapshot's human-readable name.
pub(crate) const LABEL_SNAPSHOT_NAME: &str = "malbox.snapshot-name";

/// Repository for committed snapshot images.
pub(crate) const SNAPSHOT_REPO: &str = "malbox-snapshot";
/// Repository for loaded base image archives.
pub(crate) const BASE_REPO: &str = "malbox-base";

/// Container provider (Docker/Podman Engine API).
#[derive(RegisterProvider)]
#[provider(name = "container")]
#[capability(Snapshot)]
#[capability(GuestAccess)]
pub struct ContainerProvider {
    /// Un-negotiated client built by [`ContainerProvider::new`].
    base: Docker,
    /// Version-negotiated client, created once the network is ensured.
    ready: tokio::sync::OnceCell<Docker>,
    config: ContainerProviderConfig,
    subnet: Ipv4Subnet,
    /// Serialises IP allocation with container creation.
    alloc_lock: tokio::sync::Mutex<()>,
    /// Serialises image archive loading.
    image_lock: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for ContainerProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerProvider")
            .field("config", &self.config)
            .field("subnet", &self.subnet)
            .finish_non_exhaustive()
    }
}

/// `[providers.container]` configuration.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerProviderConfig {
    /// Engine socket URL.
    #[serde(default = "default_socket")]
    pub socket: String,
    /// Bridge network machines attach to; created if missing.
    #[serde(default = "default_network")]
    pub network: String,
    /// IPv4 subnet of the network; machine IPs are assigned from it.
    #[serde(default = "default_subnet")]
    pub subnet: String,
    /// Create the network as Docker `internal` (no egress).
    #[serde(default)]
    pub internal: bool,
    /// Default OCI runtime, e.g. `"runsc"`.
    #[serde(default)]
    pub runtime: Option<String>,
}

fn default_socket() -> String {
    "unix:///var/run/docker.sock".to_string()
}

fn default_network() -> String {
    "malbox".to_string()
}

fn default_subnet() -> String {
    "172.30.0.0/24".to_string()
}

/// Per-machine overrides from `[machines.provider_config]`.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerOverrides {
    /// OCI runtime; overrides the provider default.
    pub runtime: Option<String>,
    /// Extra Linux capabilities.
    #[serde(default)]
    pub cap_add: Vec<String>,
    /// Full argv; replaces the default autostart command.
    pub command: Option<Vec<String>>,
}

impl ContainerProvider {
    /// Create the provider from its TOML configuration.
    ///
    /// Makes no engine calls; the client is version-negotiated on first use.
    ///
    /// # Errors
    ///
    /// Fails if `config` is not a valid `[providers.container]` table, its
    /// subnet is invalid, or the engine client cannot be constructed.
    pub fn new(config: &TomlValue) -> Result<Self, ContainerError> {
        let config: ContainerProviderConfig =
            malbox_machinery::provider::config::deserialize(config).map_err(|e| {
                ContainerError::Config(format!("invalid [providers.container]: {e}"))
            })?;

        if !config.socket.starts_with("unix://") {
            return Err(ContainerError::Config(
                "socket must be a unix:// URL".to_string(),
            ));
        }

        let subnet = Ipv4Subnet::parse(&config.subnet)?;
        let base = Docker::connect_with_unix(&config.socket, 120, bollard::API_DEFAULT_VERSION)?;

        Ok(Self {
            base,
            ready: tokio::sync::OnceCell::new(),
            config,
            subnet,
            alloc_lock: tokio::sync::Mutex::new(()),
            image_lock: tokio::sync::Mutex::new(()),
        })
    }

    /// Engine client, negotiated to the server's API version, with the
    /// machine network in place.
    pub(crate) async fn docker(&self) -> Result<&Docker, ContainerError> {
        self.ready
            .get_or_try_init(|| async {
                let docker = self.base.clone().negotiate_version().await?;
                network::ensure(&docker, &self.config, &self.subnet).await?;
                Ok(docker)
            })
            .await
    }
}

/// Engine container name for a machine id.
pub(crate) fn container_name(id: &MachineId) -> String {
    format!("malbox-{id}")
}

/// Machine names become container names and label values.
pub(crate) fn validate_name(name: &str) -> Result<(), ContainerError> {
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(ContainerError::Config(format!(
            "machine name '{name}' must match ^[A-Za-z0-9_.-]+$"
        )))
    }
}

/// List filter selecting malbox-managed containers.
pub(crate) fn managed_filter() -> HashMap<String, Vec<String>> {
    HashMap::from([("label".to_string(), vec![format!("{LABEL_MANAGED}=true")])])
}
