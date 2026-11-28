use crate::github::Channel;
use serde::{Deserialize, Serialize};

/// Everything the installer needs to know to perform a fresh install.
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
    /// Where the daemon binaries come from.
    pub daemon: DaemonSource,
    /// Where the front-end bundle comes from.
    pub frontend: FrontendSource,
    /// Which database to use.
    pub postgres: PostgresStrategy,
    /// Install and enable systemd user services.
    pub systemd: bool,
}

/// Origin of the daemon binaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DaemonSource {
    /// Download a prebuilt release archive.
    Prebuilt {
        /// Download URL of the release asset.
        url: String,
    },
    /// Build from the release source tarball with `InstallConfig::features`.
    Compile,
}

/// Origin of the front-end bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum FrontendSource {
    /// Download a prebuilt bundle.
    Prebuilt {
        /// Download URL of the release asset.
        url: String,
    },
    /// Build the bundle from the release source tarball.
    Compile,
}

/// How the daemon's `PostgreSQL` database is provided.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PostgresStrategy {
    /// Use an existing server.
    Existing {
        /// Connection URL of the server.
        url: String,
    },
    /// Provision a managed local server.
    Setup,
}

/// Options controlling an upgrade.
#[derive(Debug, Clone)]
pub struct UpgradeConfig {
    /// Reinstall even when already on the latest version.
    pub force: bool,
    /// New install choices to apply; `None` keeps the manifest's.
    pub reconfigure: Option<InstallConfig>,
}
