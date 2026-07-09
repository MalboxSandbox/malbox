use crate::error::{RegistryError, Result};
use crate::index::PluginMetadata;
use crate::lockfile::InstallSource;
use malbox_installer::github::{Release, ReleaseAsset};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum RefSelector {
    #[default]
    LatestRelease,
    Release(String),
    Branch(String),
    Commit(String),
}

/// How the source build checks out the tree. Mechanism, not intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRef {
    Named(String),  // branch or tag: git clone --depth 1 --branch <ref>
    Commit(String), // sha: shallow fetch-by-sha then checkout FETCH_HEAD
}

#[derive(Debug, Clone)]
pub struct PluginSpecifier {
    pub name: String,
    pub selector: RefSelector,
    pub source: SpecifierSource,
}

#[derive(Debug, Clone)]
pub enum SpecifierSource {
    Registry,
    Direct { owner: String, repo: String },
    Local { path: PathBuf },
}

#[derive(Debug, Clone)]
pub struct Platform {
    pub arch: String,
    pub os: String,
}

impl Platform {
    pub fn current() -> Self {
        Self {
            arch: std::env::consts::ARCH.to_string(),
            os: std::env::consts::OS.to_string(),
        }
    }

    pub fn asset_suffix(&self) -> String {
        format!("{}-{}", self.arch, self.os)
    }
}

#[derive(Debug)]
pub struct ResolvedPlugin {
    pub name: String,
    pub version: String,
    pub repository: String,
    pub source: InstallSource,
    pub metadata: Option<PluginMetadata>,
    pub strategy: InstallStrategy,
    pub pin: crate::lockfile::PinKind,
}

#[derive(Debug)]
pub enum InstallStrategy {
    Prebuilt {
        release: Release,
        asset: ReleaseAsset,
    },
    Source {
        clone_url: String,
        source_ref: SourceRef,
    },
    Local {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub enum RequestedStrategy {
    PrebuiltOnly,
    SourceOnly,
    #[default]
    PrebuiltWithFallback,
}

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
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| input.to_string());
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
