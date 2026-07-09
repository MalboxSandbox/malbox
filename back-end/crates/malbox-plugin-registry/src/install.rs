use crate::client::RegistryClient;
use crate::error::{RegistryError, Result};
use crate::index::Dependency;
use crate::lockfile::{InstallMethod, InstallSource, LockedPlugin, Lockfile};
use crate::resolve::{
    InstallStrategy, Platform, PluginSpecifier, RequestedStrategy, ResolvedPlugin, SpecifierSource,
    find_matching_asset,
};
use crate::source;
use malbox_installer::github::GitHubClient;
use malbox_installer::progress::ProgressObserver;
use std::path::{Path, PathBuf};

pub struct InstallOutcome {
    pub name: String,
    pub version: String,
    pub plugin_type: String,
    pub plugin_dir: PathBuf,
    pub missing_deps: Vec<Dependency>,
}

pub async fn resolve_and_install(
    specifier: &PluginSpecifier,
    registry_client: &RegistryClient,
    plugins_dir: &Path,
    platform: &Platform,
    strategy: RequestedStrategy,
    force: bool,
    observer: &dyn ProgressObserver,
) -> Result<InstallOutcome> {
    let resolved = resolve_plugin(specifier, registry_client, platform, strategy).await?;
    install_resolved_plugin(resolved, plugins_dir, force, observer).await
}

pub async fn resolve_plugin(
    specifier: &PluginSpecifier,
    registry_client: &RegistryClient,
    platform: &Platform,
    strategy: RequestedStrategy,
) -> Result<ResolvedPlugin> {
    if let SpecifierSource::Local { path } = &specifier.source {
        let manifest_path = path.join("plugin.toml");
        if !manifest_path.exists() {
            return Err(RegistryError::InvalidPlugin {
                path: path.clone(),
                reason: "plugin.toml not found".into(),
            });
        }
        let manifest = malbox_plugin_manifest::parse_manifest(&manifest_path)?;
        return Ok(ResolvedPlugin {
            name: manifest.plugin.name.clone(),
            version: manifest.plugin.version.clone(),
            repository: String::new(),
            source: InstallSource::Local,
            metadata: None,
            strategy: InstallStrategy::Local { path: path.clone() },
            pin: crate::lockfile::PinKind::Local,
        });
    }

    let (owner, repo, metadata) = match &specifier.source {
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

            (owner.to_string(), repo.to_string(), Some(meta))
        }
        SpecifierSource::Direct { owner, repo } => (owner.clone(), repo.clone(), None),
        SpecifierSource::Local { .. } => unreachable!("local handled above"),
    };

    let source = match &specifier.source {
        SpecifierSource::Registry => InstallSource::Registry,
        SpecifierSource::Direct { .. } => InstallSource::Direct,
        SpecifierSource::Local { .. } => unreachable!("local handled above"),
    };

    let clone_url = format!("https://github.com/{owner}/{repo}.git");

    use crate::lockfile::PinKind;
    use crate::resolve::{RefSelector, SourceRef};

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
            let gh = GitHubClient::new(&owner, &repo)?;
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
                    source_ref: SourceRef::Named(release.tag_name.clone()),
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

            let version = malbox_installer::github::Release::version_from_tag(match &strat {
                InstallStrategy::Prebuilt { release, .. } => release.tag_name.as_str(),
                InstallStrategy::Source { source_ref, .. } => match source_ref {
                    SourceRef::Named(r) => r.as_str(),
                    SourceRef::Commit(c) => c.as_str(),
                },
                InstallStrategy::Local { .. } => unreachable!("local handled above"),
            })
            .to_string();

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

pub async fn install_resolved_plugin(
    resolved: ResolvedPlugin,
    plugins_dir: &Path,
    force: bool,
    observer: &dyn ProgressObserver,
) -> Result<InstallOutcome> {
    let dest = plugins_dir.join(&resolved.name);
    if dest.exists() && !force {
        return Err(RegistryError::AlreadyInstalled(resolved.name.clone()));
    }

    let pin = resolved.pin.clone();

    let local_path = match &resolved.strategy {
        InstallStrategy::Local { path } => Some(path.to_string_lossy().to_string()),
        _ => None,
    };

    let (plugin_type, plugin_dir, commit) = match &resolved.strategy {
        InstallStrategy::Prebuilt { release, asset } => {
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
            observer.step_completed(
                "Downloading asset",
                &format!("{:.1} MB", bytes.len() as f64 / 1_048_576.0),
            );

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
            let (plugin_type, plugin_dir) =
                source::install_from_local(path, &resolved.name, plugins_dir, observer).await?;
            (plugin_type, plugin_dir, None)
        }
    };

    let lockfile_path = Lockfile::lockfile_path(plugins_dir);
    let mut lockfile = Lockfile::load(&lockfile_path)?;

    let install_method = match &resolved.strategy {
        InstallStrategy::Prebuilt { .. } => InstallMethod::Prebuilt,
        InstallStrategy::Source { .. } => InstallMethod::Source,
        InstallStrategy::Local { .. } => InstallMethod::Source,
    };

    let (asset_name, checksum) = match &resolved.strategy {
        InstallStrategy::Prebuilt { asset, .. } => (Some(asset.name.clone()), None),
        InstallStrategy::Source { .. } => (None, None),
        InstallStrategy::Local { .. } => (None, None),
    };

    lockfile.plugins.insert(
        resolved.name.clone(),
        LockedPlugin {
            version: resolved.version.clone(),
            repository: resolved.repository.clone(),
            source: resolved.source,
            install_method,
            asset: asset_name,
            checksum,
            installed_at: chrono::Utc::now().to_rfc3339(),
            pin,
            commit,
            path: local_path,
        },
    );
    lockfile.write(&lockfile_path)?;

    let missing_deps = if let Some(ref meta) = resolved.metadata {
        check_soft_deps(&meta.requires, &lockfile)
            .into_iter()
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    Ok(InstallOutcome {
        name: resolved.name,
        version: resolved.version,
        plugin_type,
        plugin_dir,
        missing_deps,
    })
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

    match dirs.len() {
        0 => Err(RegistryError::InvalidPlugin {
            path: base.to_path_buf(),
            reason: "empty archive".into(),
        }),
        1 => Ok(dirs.into_iter().next().unwrap()),
        _ => Ok(base.to_path_buf()),
    }
}

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

pub fn check_soft_deps<'a>(requires: &'a [Dependency], lockfile: &Lockfile) -> Vec<&'a Dependency> {
    requires
        .iter()
        .filter(|dep| !lockfile.plugins.contains_key(&dep.name))
        .collect()
}
