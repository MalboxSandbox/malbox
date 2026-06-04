use crate::github::Channel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallConfig {
    /// Virtualization providers enabled in the generated daemon config
    /// (e.g. "libvirt").
    pub providers: Vec<String>,
    /// Machine provisioners compiled into the daemon (e.g. "ansible").
    pub provisioners: Vec<String>,
    /// Cargo feature set the daemon binaries carry. Recorded in the manifest
    /// so upgrades can replay a from-source build with identical features.
    pub features: Vec<String>,
    /// Release channel this installation tracks.
    pub channel: Channel,
    pub daemon: DaemonSource,
    pub frontend: FrontendSource,
    pub postgres: PostgresStrategy,
    pub systemd: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonSource {
    Prebuilt {
        url: String,
    },
    /// Build from the release source tarball with `InstallConfig::features`.
    Compile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FrontendSource {
    Prebuilt { url: String },
    Compile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PostgresStrategy {
    Existing { url: String },
    Setup,
}

#[derive(Debug, Clone)]
pub struct UpgradeConfig {
    pub force: bool,
    pub reconfigure: Option<InstallConfig>,
}
