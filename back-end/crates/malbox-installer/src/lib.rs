mod archive;
pub mod build_parse;
pub mod config;
pub mod error;
pub mod features;
pub mod github;
pub mod install;
pub mod manifest;
pub mod progress;
pub mod rebuild;
pub mod steps;
pub mod upgrade;

pub use config::{DaemonSource, FrontendSource, InstallConfig, PostgresStrategy, UpgradeConfig};
pub use error::{InstallError, Result, Step};
pub use features::{DAEMON_FEATURES, FeatureKind, default_features};
pub use github::Channel;
pub use progress::{NullObserver, ProgressObserver};

/// Canonical GitHub coordinates for malbox releases. Single source of truth so
/// the installer, upgrader and source-tarball fallbacks never drift apart.
pub const GITHUB_OWNER: &str = "DualHorizon";
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
