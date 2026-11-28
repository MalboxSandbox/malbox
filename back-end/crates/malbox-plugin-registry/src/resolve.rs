use crate::error::{RegistryError, Result};
use crate::index::PluginMetadata;
use crate::lockfile::InstallSource;
use malbox_installer::github::{Release, ReleaseAsset};
use std::path::PathBuf;

/// Which ref of a plugin repository the user asked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum RefSelector {
    /// The newest published release.
    #[default]
    LatestRelease,
    /// A specific release version.
    Release(String),
    /// The head of a branch, built from source.
    Branch(String),
    /// An exact commit, built from source.
    Commit(String),
}

/// How the source build checks out the tree. Mechanism, not intent.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceRef {
    /// Branch or tag: `git clone --depth 1 --branch <ref>`.
    Named(String),
    /// Commit SHA: shallow fetch-by-sha then checkout `FETCH_HEAD`.
    Commit(String),
}

/// Parsed install request: which plugin, from where, at which ref.
#[derive(Debug, Clone)]
pub struct PluginSpecifier {
    /// Plugin name; for local paths a placeholder until the manifest is read.
    pub name: String,
    /// Requested ref.
    pub selector: RefSelector,
    /// Where to look the plugin up.
    pub source: SpecifierSource,
}

/// Where a specifier points.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum SpecifierSource {
    /// Look up the name in the plugin registry.
    Registry,
    /// A GitHub repository given directly as `owner/repo`.
    Direct {
        /// GitHub repository owner.
        owner: String,
        /// GitHub repository name.
        repo: String,
    },
    /// A plugin directory on the local filesystem.
    Local {
        /// Path to the plugin directory, with `~` expanded.
        path: PathBuf,
    },
}

/// Target platform used to pick prebuilt release assets.
#[derive(Debug, Clone)]
pub struct Platform {
    /// CPU architecture, as in `std::env::consts::ARCH`.
    pub arch: String,
    /// Operating system, as in `std::env::consts::OS`.
    pub os: String,
}

impl Platform {
    /// Platform of the running binary.
    #[must_use]
    pub fn current() -> Self {
        Self {
            arch: std::env::consts::ARCH.to_string(),
            os: std::env::consts::OS.to_string(),
        }
    }

    /// Asset name suffix `<arch>-<os>` used in release tarball names.
    #[must_use]
    pub fn asset_suffix(&self) -> String {
        format!("{}-{}", self.arch, self.os)
    }
}

/// A specifier resolved to a concrete version, source and install strategy.
#[derive(Debug)]
pub struct ResolvedPlugin {
    /// Canonical plugin name.
    pub name: String,
    /// Version, branch name or commit that will be installed.
    pub version: String,
    /// GitHub `owner/repo`; empty for local installs.
    pub repository: String,
    /// How the plugin was located.
    pub source: InstallSource,
    /// Registry metadata, present only for registry installs.
    pub metadata: Option<PluginMetadata>,
    /// How the plugin will be installed.
    pub strategy: InstallStrategy,
    /// Ref the installation will track.
    pub pin: crate::lockfile::PinKind,
}

/// Concrete installation mechanism chosen during resolution.
#[derive(Debug)]
#[non_exhaustive]
pub enum InstallStrategy {
    /// Download a prebuilt release asset.
    Prebuilt {
        /// Release the asset belongs to.
        release: Release,
        /// Asset matching the current platform.
        asset: ReleaseAsset,
    },
    /// Clone and build from source.
    Source {
        /// HTTPS clone URL of the repository.
        clone_url: String,
        /// Ref to check out.
        source_ref: SourceRef,
    },
    /// Build or copy from a local directory.
    Local {
        /// Local plugin directory.
        path: PathBuf,
    },
}

/// User preference between prebuilt assets and source builds for releases.
#[derive(Debug, Clone, Copy, Default)]
#[non_exhaustive]
pub enum RequestedStrategy {
    /// Require a prebuilt asset; fail if none matches the platform.
    PrebuiltOnly,
    /// Always build from source.
    SourceOnly,
    /// Use a prebuilt asset when available, else build from source.
    #[default]
    PrebuiltWithFallback,
}

/// Parse `name`, `name@version`, `owner/repo`, `owner/repo@version`, or a local
/// path (`/`, `./`, `../`, `~`) into a [`PluginSpecifier`].
///
/// # Errors
///
/// Returns [`RegistryError::InvalidSpecifier`] for empty input, an empty version
/// after `@`, an empty name, or an empty owner or repo.
pub fn parse_specifier(input: &str) -> Result<PluginSpecifier> {
    if input.is_empty() {
        return Err(RegistryError::InvalidSpecifier(input.to_string()));
    }

    if input.starts_with('/')
        || input.starts_with("./")
        || input.starts_with("../")
        || input.starts_with('~')
    {
        let expanded = if let Some(rest) = input.strip_prefix('~') {
            match std::env::var("HOME") {
                Ok(home) => PathBuf::from(home).join(rest.trim_start_matches('/')),
                Err(_) => PathBuf::from(input),
            }
        } else {
            PathBuf::from(input)
        };
        // Placeholder; the canonical name is read from the directory's
        // plugin.toml during resolution.
        let name = expanded
            .file_name()
            .map_or_else(|| input.to_string(), |n| n.to_string_lossy().to_string());
        return Ok(PluginSpecifier {
            name,
            selector: RefSelector::LatestRelease,
            source: SpecifierSource::Local { path: expanded },
        });
    }

    let (name_part, version) = match input.split_once('@') {
        Some((name, ver)) if !ver.is_empty() => (name, Some(ver.to_string())),
        Some((_, _)) => return Err(RegistryError::InvalidSpecifier(input.to_string())),
        None => (input, None),
    };

    if name_part.is_empty() {
        return Err(RegistryError::InvalidSpecifier(input.to_string()));
    }

    let (source, name) = if let Some((owner, repo)) = name_part.split_once('/') {
        if owner.is_empty() || repo.is_empty() {
            return Err(RegistryError::InvalidSpecifier(input.to_string()));
        }
        (
            SpecifierSource::Direct {
                owner: owner.to_string(),
                repo: repo.to_string(),
            },
            repo.to_string(),
        )
    } else {
        (SpecifierSource::Registry, name_part.to_string())
    };

    let selector = match version {
        Some(v) => RefSelector::Release(v),
        None => RefSelector::LatestRelease,
    };

    Ok(PluginSpecifier {
        name,
        selector,
        source,
    })
}

/// Fold the `--release` / `--branch` / `--rev` flags into a final selector.
/// clap enforces that at most one flag is set and that `--branch`/`--rev` do
/// not combine with `--prebuilt`; this function only has to reconcile a flag
/// with a `name@version` already baked into `parsed`.
///
/// # Errors
///
/// Returns [`RegistryError::ConflictingSelectors`] if more than one flag is set,
/// or if `--release` is combined with a `name@version` specifier.
pub fn resolve_selector(
    parsed: RefSelector,
    release: Option<String>,
    branch: Option<String>,
    rev: Option<String>,
) -> Result<RefSelector> {
    let flag = match (release, branch, rev) {
        (None, None, None) => None,
        (Some(v), None, None) => Some(RefSelector::Release(v)),
        (None, Some(b), None) => Some(RefSelector::Branch(b)),
        (None, None, Some(r)) => Some(RefSelector::Commit(r)),
        _ => {
            return Err(RegistryError::ConflictingSelectors(
                "use only one of --release, --branch, --rev".to_string(),
            ));
        }
    };

    match (parsed, flag) {
        (parsed, None) => Ok(parsed),
        (RefSelector::LatestRelease, Some(flag)) => Ok(flag),
        (RefSelector::Release(_), Some(_)) => Err(RegistryError::ConflictingSelectors(
            "specify the version with --release or name@version, not both".to_string(),
        )),
        // parse_specifier only ever produces LatestRelease or Release.
        (other, Some(_)) => Ok(other),
    }
}

/// Find the release asset named `<plugin>-<tag>-<arch>-<os>.tar.gz`.
#[must_use]
pub fn find_matching_asset<'a>(
    release: &'a Release,
    plugin_name: &str,
    platform: &Platform,
) -> Option<&'a ReleaseAsset> {
    let expected = format!(
        "{}-{}-{}.tar.gz",
        plugin_name,
        release.tag_name,
        platform.asset_suffix()
    );
    release.assets.iter().find(|a| a.name == expected)
}
