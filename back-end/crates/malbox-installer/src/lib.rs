mod archive;
pub mod config;
pub mod error;
pub mod github;
pub mod install;
pub mod manifest;
pub mod progress;
pub mod steps;
pub mod upgrade;

pub use config::{DaemonSource, FrontendSource, InstallConfig, PostgresStrategy, UpgradeConfig};
pub use error::{InstallError, Result, Step};
pub use github::Channel;
pub use progress::{InstallProgress, NoopProgress};

/// Canonical GitHub coordinates for malbox releases. Single source of truth so
/// the installer, upgrader and source-tarball fallbacks never drift apart.
pub const GITHUB_OWNER: &str = "DualHorizon";
pub const GITHUB_REPO: &str = "malbox";

/// Cargo features baked into the prebuilt `malboxctl` release binaries.
///
/// The install wizard compares the user's selection against this set to
/// decide whether the stock binary is equivalent to compiling, and upgrades
/// fall back to it for manifests that predate feature recording.
pub const DEFAULT_DAEMON_FEATURES: &[&str] = &["provider-libvirt", "provisioner-ansible"];

/// Resolve an XDG directory with a `$HOME`-based fallback. Never falls back
/// to a literal `~` path - the OS does not expand those.
pub(crate) fn xdg_dir(
    primary: Option<std::path::PathBuf>,
    home_suffix: &str,
) -> Result<std::path::PathBuf> {
    primary
        .or_else(|| dirs::home_dir().map(|home| home.join(home_suffix)))
        .ok_or_else(|| InstallError::Environment("could not determine home directory".to_string()))
}
