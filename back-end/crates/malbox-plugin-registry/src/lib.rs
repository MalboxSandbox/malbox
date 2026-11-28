//! Discovery, installation, update and removal of malbox plugins from the
//! GitHub-hosted plugin registry, GitHub releases, or local directories.

/// HTTP client and on-disk cache for registry index files.
pub mod client;
/// Detection of registry dependencies missing from the lockfile.
pub mod deps;
/// Registry error type and plugin name validation.
pub mod error;
/// Serde models of the registry index and per-plugin metadata.
pub mod index;
/// Download, build and install of resolved plugins.
pub mod install;
/// `plugins.lock` model recording installed plugins and their pins.
pub mod lockfile;
/// Uninstallation of plugins and lockfile cleanup.
pub mod remove;
/// Parsing of install specifiers and resolution to concrete sources.
pub mod resolve;
/// Fetching plugin sources from GitHub releases, archives and git.
pub mod source;
/// Checks for newer versions of installed plugins.
pub mod update;

pub use error::{RegistryError, Result};
