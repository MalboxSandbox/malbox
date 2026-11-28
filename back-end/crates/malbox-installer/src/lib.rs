//! Installs, upgrades, rebuilds, and rolls back a malbox deployment from GitHub releases.

mod archive;
pub mod build_parse;
/// User choices describing what to install and how.
pub mod config;
/// Install errors and the ordered install steps.
pub mod error;
pub mod features;
/// GitHub release lookup and verified asset downloads.
pub mod github;
/// Fresh install and resume of an interrupted install.
pub mod install;
/// Persistent record of what is installed where.
pub mod manifest;
/// Progress reporting hooks for install front ends.
pub mod progress;
/// Rebuilding the daemon from source with a different feature set.
pub mod rebuild;
/// Individual install steps.
pub mod steps;
/// Upgrading to a newer release and rolling back.
pub mod upgrade;

pub use config::{DaemonSource, FrontendSource, InstallConfig, PostgresStrategy, UpgradeConfig};
pub use error::{InstallError, Result, Step};
pub use features::{DAEMON_FEATURES, FeatureKind, default_features};
pub use github::Channel;
pub use progress::{NullObserver, ProgressObserver};

/// Canonical GitHub coordinates for malbox releases. Single source of truth so
/// the installer, upgrader and source-tarball fallbacks never drift apart.
pub const GITHUB_OWNER: &str = "MalboxSandbox";
/// GitHub repository name of malbox releases; see [`GITHUB_OWNER`].
pub const GITHUB_REPO: &str = "malbox";

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
