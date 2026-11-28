use crate::client::RegistryClient;
use crate::error::{RegistryError, Result};
use crate::index::{Dependency, PluginMetadata};
use crate::lockfile::{InstallMethod, InstallSource, LockedPlugin, Lockfile, PinKind};
use crate::resolve::{
    InstallStrategy, Platform, PluginSpecifier, RefSelector, RequestedStrategy, ResolvedPlugin,
    SourceRef, SpecifierSource, find_matching_asset,
};
use crate::source;
use malbox_installer::github::{GitHubClient, Release, ReleaseAsset};
use malbox_installer::progress::ProgressObserver;
use std::path::{Path, PathBuf};

/// What to do when the plugin's target directory already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExistingInstall {
    /// Fail with [`RegistryError::AlreadyInstalled`].
    Reject,
    /// Replace the existing installation.
    Replace,
}

/// Summary of a completed installation.
#[derive(Debug)]
pub struct InstallOutcome {
    /// Installed plugin name.
    pub name: String,
    /// Installed version, tag-derived version, branch name or commit.
    pub version: String,
    /// Plugin type label from the manifest (`host` or `guest`).
    pub plugin_type: String,
    /// Directory the plugin was installed into.
    pub plugin_dir: PathBuf,
    /// Declared registry dependencies that are not installed.
    pub missing_deps: Vec<Dependency>,
}

/// Resolve `specifier` to a concrete source and install it into `plugins_dir`.
///
/// # Errors
///
/// Returns any error from [`resolve_plugin`] or [`install_resolved_plugin`].
pub async fn resolve_and_install(
    specifier: &PluginSpecifier,
    registry_client: &RegistryClient,
    plugins_dir: &Path,
    platform: &Platform,
    strategy: RequestedStrategy,
    existing: ExistingInstall,
    observer: &dyn ProgressObserver,
) -> Result<InstallOutcome> {
    let resolved = resolve_plugin(specifier, registry_client, platform, strategy).await?;
    install_resolved_plugin(resolved, plugins_dir, existing, observer).await
}

/// Resolve a specifier to a concrete install strategy, version and pin without
/// installing anything.
///
/// # Errors
///
/// Returns [`RegistryError::InvalidPlugin`] or a manifest error for a bad local
/// directory, [`RegistryError::PluginNotFound`] for an unknown registry name,
/// [`RegistryError::NoPlatformAsset`] when a prebuilt-only install has no asset,
/// and network/GitHub errors from the registry or release lookups.
pub async fn resolve_plugin(
    specifier: &PluginSpecifier,
    registry_client: &RegistryClient,
    platform: &Platform,
    strategy: RequestedStrategy,
) -> Result<ResolvedPlugin> {
    let (owner, repo, metadata, source) = match &specifier.source {
        SpecifierSource::Local { path } => return resolve_local(path),
        SpecifierSource::Registry => {
            let index = registry_client.fetch_index().await?;
            let entry = index
                .find(&specifier.name)
                .ok_or_else(|| RegistryError::PluginNotFound(specifier.name.clone()))?;

            let (owner, repo) = entry.repository.split_once('/').ok_or_else(|| {
                RegistryError::GitHub(format!("invalid repository: {}", entry.repository))
            })?;

            let meta = registry_client
                .fetch_plugin_metadata(&specifier.name)
                .await?;

            (
                owner.to_string(),
                repo.to_string(),
                Some(meta),
                InstallSource::Registry,
            )
        }
        SpecifierSource::Direct { owner, repo } => {
            (owner.clone(), repo.clone(), None, InstallSource::Direct)
        }
    };

    let clone_url = format!("https://github.com/{owner}/{repo}.git");

    let (install_strategy, version, pin) = match &specifier.selector {
        RefSelector::Branch(name) => (
            InstallStrategy::Source {
                clone_url,
                source_ref: SourceRef::Named(name.clone()),
            },
            name.clone(),
            PinKind::Branch { name: name.clone() },
        ),
        RefSelector::Commit(sha) => (
            InstallStrategy::Source {
                clone_url,
                source_ref: SourceRef::Commit(sha.clone()),
            },
            sha.clone(),
            PinKind::Commit,
        ),
        RefSelector::LatestRelease | RefSelector::Release(_) => {
            let (strat, version) =
                resolve_release(specifier, &owner, &repo, clone_url, platform, strategy).await?;
            (strat, version, PinKind::Release)
        }
    };

    Ok(ResolvedPlugin {
        name: specifier.name.clone(),
        version,
        repository: format!("{owner}/{repo}"),
        source,
        metadata,
        strategy: install_strategy,
        pin,
    })
}

/// Resolve a local plugin directory using the name and version from its manifest.
fn resolve_local(path: &Path) -> Result<ResolvedPlugin> {
    let manifest_path = path.join("plugin.toml");
    if !manifest_path.exists() {
        return Err(RegistryError::InvalidPlugin {
            path: path.to_path_buf(),
            reason: "plugin.toml not found".into(),
        });
    }
    let manifest = malbox_plugin_manifest::parse_manifest(&manifest_path)?;
    Ok(ResolvedPlugin {
        name: manifest.plugin.name,
        version: manifest.plugin.version,
        repository: String::new(),
        source: InstallSource::Local,
        metadata: None,
        strategy: InstallStrategy::Local {
            path: path.to_path_buf(),
        },
        pin: PinKind::Local,
    })
}

/// Pick the release for a release selector and choose prebuilt vs source per
/// `strategy`, returning the strategy and the tag-derived version.
async fn resolve_release(
    specifier: &PluginSpecifier,
    owner: &str,
    repo: &str,
    clone_url: String,
    platform: &Platform,
    strategy: RequestedStrategy,
) -> Result<(InstallStrategy, String)> {
    let gh = GitHubClient::new(owner, repo)?;
    let release = match &specifier.selector {
        RefSelector::Release(v) => gh.release_for_version(v).await?,
        _ => {
            gh.latest_release(malbox_installer::Channel::Nightly)
                .await?
        }
    };

    let strat = match strategy {
        RequestedStrategy::SourceOnly => InstallStrategy::Source {
            clone_url,
            source_ref: SourceRef::Named(release.tag_name),
        },
        RequestedStrategy::PrebuiltOnly => {
            let asset = find_matching_asset(&release, &specifier.name, platform)
                .ok_or_else(|| RegistryError::NoPlatformAsset {
                    plugin: specifier.name.clone(),
                    platform: platform.asset_suffix(),
                })?
                .clone();
            InstallStrategy::Prebuilt { release, asset }
        }
        RequestedStrategy::PrebuiltWithFallback => {
            match find_matching_asset(&release, &specifier.name, platform).cloned() {
                Some(asset) => InstallStrategy::Prebuilt { release, asset },
                None => InstallStrategy::Source {
                    clone_url,
                    source_ref: SourceRef::Named(release.tag_name.clone()),
                },
            }
        }
    };

    let version = Release::version_from_tag(match &strat {
        InstallStrategy::Prebuilt { release, .. } => release.tag_name.as_str(),
        InstallStrategy::Source { source_ref, .. } => match source_ref {
            SourceRef::Named(r) => r.as_str(),
            SourceRef::Commit(c) => c.as_str(),
        },
        InstallStrategy::Local { .. } => unreachable!("local handled above"),
    })
    .to_string();

    Ok((strat, version))
}

/// Install an already-resolved plugin into `plugins_dir` and record it in the lockfile.
///
/// # Errors
///
/// Returns [`RegistryError::AlreadyInstalled`] if the target exists and `existing`
/// is [`ExistingInstall::Reject`]; otherwise download, checksum, extraction, build,
/// manifest validation, filesystem and lockfile errors.
pub async fn install_resolved_plugin(
    resolved: ResolvedPlugin,
    plugins_dir: &Path,
    existing: ExistingInstall,
    observer: &dyn ProgressObserver,
) -> Result<InstallOutcome> {
    let dest = plugins_dir.join(&resolved.name);
    if dest.exists() && existing == ExistingInstall::Reject {
        return Err(RegistryError::AlreadyInstalled(resolved.name.clone()));
    }

    let pin = resolved.pin.clone();

    let local_path = match &resolved.strategy {
        InstallStrategy::Local { path } => Some(path.to_string_lossy().to_string()),
        _ => None,
    };

    let (plugin_type, installed_dir, commit) = match &resolved.strategy {
        InstallStrategy::Prebuilt { release, asset } => {
            let (ptype, dest) =
                install_prebuilt(&resolved, release, asset, plugins_dir, observer).await?;
            (ptype, dest, None)
        }
        InstallStrategy::Source {
            clone_url,
            source_ref,
        } => {
            let outcome = source::build_from_source(
                clone_url,
                source_ref,
                &resolved.name,
                plugins_dir,
                observer,
            )
            .await?;
            (
                outcome.plugin_type,
                outcome.plugin_dir,
                Some(outcome.commit),
            )
        }
        InstallStrategy::Local { path } => {
            let (ptype, local_dir) =
                source::install_from_local(path, &resolved.name, plugins_dir, observer).await?;
            (ptype, local_dir, None)
        }
    };

    let (install_method, asset_name) = match &resolved.strategy {
        InstallStrategy::Prebuilt { asset, .. } => {
            (InstallMethod::Prebuilt, Some(asset.name.clone()))
        }
        InstallStrategy::Source { .. } | InstallStrategy::Local { .. } => {
            (InstallMethod::Source, None)
        }
    };

    let lockfile = record_install(
        plugins_dir,
        &resolved,
        LockedPlugin {
            version: resolved.version.clone(),
            repository: resolved.repository.clone(),
            source: resolved.source.clone(),
            install_method,
            asset: asset_name,
            checksum: None,
            installed_at: chrono::Utc::now().to_rfc3339(),
            pin,
            commit,
            path: local_path,
        },
    )?;

    let missing_deps = missing_deps_of(resolved.metadata.as_ref(), &lockfile);

    Ok(InstallOutcome {
        name: resolved.name,
        version: resolved.version,
        plugin_type,
        plugin_dir: installed_dir,
        missing_deps,
    })
}

/// Insert `entry` for the resolved plugin into the lockfile and persist it.
fn record_install(
    plugins_dir: &Path,
    resolved: &ResolvedPlugin,
    entry: LockedPlugin,
) -> Result<Lockfile> {
    let lockfile_path = Lockfile::lockfile_path(plugins_dir);
    let mut lockfile = Lockfile::load(&lockfile_path)?;
    lockfile.plugins.insert(resolved.name.clone(), entry);
    lockfile.write(&lockfile_path)?;
    Ok(lockfile)
}

/// Registry dependencies declared in `metadata` that the lockfile lacks.
fn missing_deps_of(metadata: Option<&PluginMetadata>, lockfile: &Lockfile) -> Vec<Dependency> {
    metadata.map_or_else(Vec::new, |meta| {
        check_soft_deps(&meta.requires, lockfile)
            .into_iter()
            .cloned()
            .collect()
    })
}

/// Download, verify and extract a prebuilt release asset into the plugins
/// directory, returning the plugin type label and installed directory.
async fn install_prebuilt(
    resolved: &ResolvedPlugin,
    release: &Release,
    asset: &ReleaseAsset,
    plugins_dir: &Path,
    observer: &dyn ProgressObserver,
) -> Result<(String, PathBuf)> {
    let (owner, repo) = resolved
        .repository
        .split_once('/')
        .ok_or_else(|| RegistryError::GitHub("invalid repository".into()))?;

    let gh = GitHubClient::new(owner, repo)?;

    observer.step_started("Downloading asset");
    let bytes = gh
        .download_verified(release, &asset.browser_download_url, &mut |done, total| {
            observer.download_progress(done, total, "plugin archive");
        })
        .await?;
    observer.step_completed("Downloading asset", &format_megabytes(bytes.len()));

    std::fs::create_dir_all(plugins_dir)?;

    observer.step_started("Extracting archive");
    let temp_dir = tempfile::tempdir_in(plugins_dir)?;
    extract_tarball(&bytes, temp_dir.path())?;
    let extracted_dir = find_extracted_subdir(temp_dir.path())?;
    observer.step_completed("Extracting archive", "");

    observer.step_started("Validating plugin manifest");
    let dest = plugins_dir.join(&resolved.name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::rename(&extracted_dir, &dest)?;
    let manifest = validate_extracted_plugin(&dest)?;
    let ptype = format!("{:?}", manifest.plugin.plugin_type).to_lowercase();
    observer.step_completed("Validating plugin manifest", "");

    observer.step_started("Installing to plugins directory");
    observer.step_completed("Installing to plugins directory", "");

    Ok((ptype, dest))
}

/// Format a byte count as mebibytes with one decimal, e.g. `"3.2 MB"`.
#[expect(
    clippy::as_conversions,
    clippy::cast_precision_loss,
    reason = "display-only size; precision loss above 2^52 bytes is irrelevant"
)]
fn format_megabytes(len: usize) -> String {
    format!("{:.1} MB", len as f64 / 1_048_576.0)
}

fn extract_tarball(bytes: &[u8], dest: &Path) -> Result<()> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(dest)
        .map_err(|e| RegistryError::InvalidPlugin {
            path: dest.to_path_buf(),
            reason: format!("failed to extract tarball: {e}"),
        })
}

fn find_extracted_subdir(base: &Path) -> Result<PathBuf> {
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(base)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            dirs.push(entry.path());
        }
    }

    match dirs.as_slice() {
        [] => Err(RegistryError::InvalidPlugin {
            path: base.to_path_buf(),
            reason: "empty archive".into(),
        }),
        [only] => Ok(only.clone()),
        _ => Ok(base.to_path_buf()),
    }
}

/// Load and validate the `plugin.toml` inside an extracted plugin directory.
///
/// # Errors
///
/// Returns [`RegistryError::InvalidPlugin`] if `plugin.toml` is missing, or
/// [`RegistryError::Manifest`] if it fails to parse or validate.
pub fn validate_extracted_plugin(
    plugin_dir: &Path,
) -> Result<malbox_plugin_manifest::PluginManifest> {
    let manifest_path = plugin_dir.join("plugin.toml");
    if !manifest_path.exists() {
        return Err(RegistryError::InvalidPlugin {
            path: plugin_dir.to_path_buf(),
            reason: "plugin.toml not found".into(),
        });
    }
    let manifest = malbox_plugin_manifest::parse_manifest(&manifest_path)?;
    Ok(manifest)
}

/// Return the dependencies in `requires` that are not present in `lockfile`.
#[must_use]
pub fn check_soft_deps<'a>(requires: &'a [Dependency], lockfile: &Lockfile) -> Vec<&'a Dependency> {
    requires
        .iter()
        .filter(|dep| !lockfile.plugins.contains_key(&dep.name))
        .collect()
}
