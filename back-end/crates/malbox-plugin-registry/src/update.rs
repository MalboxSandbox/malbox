use crate::client::RegistryClient;
use crate::error::{RegistryError, Result};
use crate::install::install_resolved_plugin;
use crate::lockfile::{InstallMethod, InstallSource, LockedPlugin, Lockfile};
use crate::resolve::{Platform, PluginSpecifier, RefSelector, RequestedStrategy, SpecifierSource};
use malbox_installer::github::GitHubClient;
use malbox_installer::progress::NullObserver;

pub struct UpdateOutcome {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub updated: bool,
}

pub fn needs_update(current: &str, latest: &str) -> bool {
    let Ok(current) = semver::Version::parse(current) else {
        return false;
    };
    let Ok(latest) = semver::Version::parse(latest) else {
        return false;
    };
    latest > current
}

pub struct UpdateCheck {
    pub selector: RefSelector,
    pub should_update: bool,
    pub latest: String,
}

/// Determine whether a tracked plugin has an available update, keyed on its
/// recorded pin. Does not install anything.
pub async fn check_for_update(locked: &LockedPlugin, force: bool) -> Result<UpdateCheck> {
    use crate::lockfile::PinKind;

    let (owner, repo) = locked.repository.split_once('/').ok_or_else(|| {
        RegistryError::GitHub(format!("invalid repository: {}", locked.repository))
    })?;
    let clone_url = format!("https://github.com/{owner}/{repo}.git");

    match &locked.pin {
        PinKind::Release => {
            let gh = GitHubClient::new(owner, repo)?;
            let release = gh
                .latest_release(malbox_installer::Channel::Nightly)
                .await?;
            let latest = release.version().to_string();
            let should_update = force || needs_update(&locked.version, &latest);
            Ok(UpdateCheck {
                selector: RefSelector::LatestRelease,
                should_update,
                latest,
            })
        }
        PinKind::Branch { name } => {
            let head = crate::source::remote_branch_head(&clone_url, name).await?;
            let changed = locked.commit.as_deref() != Some(head.as_str());
            Ok(UpdateCheck {
                selector: RefSelector::Branch(name.clone()),
                should_update: force || changed,
                latest: head,
            })
        }
        PinKind::Commit => {
            // Pinned to an exact commit; only a forced rebuild does anything.
            let sha = locked
                .commit
                .clone()
                .unwrap_or_else(|| locked.version.clone());
            Ok(UpdateCheck {
                selector: RefSelector::Commit(sha.clone()),
                should_update: force,
                latest: sha,
            })
        }
        PinKind::Local => Ok(UpdateCheck {
            selector: RefSelector::LatestRelease,
            should_update: false,
            latest: locked.version.clone(),
        }),
    }
}

pub async fn update_plugin(
    name: &str,
    lockfile: &Lockfile,
    registry_client: &RegistryClient,
    plugins_dir: &std::path::Path,
    platform: &Platform,
    force: bool,
) -> Result<UpdateOutcome> {
    let locked = lockfile
        .plugins
        .get(name)
        .ok_or_else(|| RegistryError::NotInstalled(name.to_string()))?;

    let (owner, repo) = locked.repository.split_once('/').ok_or_else(|| {
        RegistryError::GitHub(format!("invalid repository: {}", locked.repository))
    })?;

    let check = check_for_update(locked, force).await?;
    if !check.should_update {
        return Ok(UpdateOutcome {
            name: name.to_string(),
            old_version: locked.version.clone(),
            new_version: check.latest,
            updated: false,
        });
    }

    let strategy = match locked.install_method {
        InstallMethod::Prebuilt => RequestedStrategy::PrebuiltOnly,
        InstallMethod::Source => RequestedStrategy::SourceOnly,
    };

    let specifier = PluginSpecifier {
        name: name.to_string(),
        selector: check.selector,
        source: match locked.source {
            InstallSource::Registry => SpecifierSource::Registry,
            InstallSource::Direct => SpecifierSource::Direct {
                owner: owner.to_string(),
                repo: repo.to_string(),
            },
            // The CLI skips local plugins before calling update; defensive guard.
            InstallSource::Local => {
                return Err(RegistryError::InvalidPlugin {
                    path: std::path::PathBuf::from(locked.path.clone().unwrap_or_default()),
                    reason: "local plugins cannot be updated; reinstall from the source path"
                        .into(),
                });
            }
        },
    };

    let resolved =
        crate::install::resolve_plugin(&specifier, registry_client, platform, strategy).await?;
    let old_version = locked.version.clone();
    let new_version = resolved.version.clone();

    install_resolved_plugin(resolved, plugins_dir, true, &NullObserver).await?;

    Ok(UpdateOutcome {
        name: name.to_string(),
        old_version,
        new_version,
        updated: true,
    })
}
