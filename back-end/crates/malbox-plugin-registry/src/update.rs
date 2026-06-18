use crate::client::RegistryClient;
use crate::error::{RegistryError, Result};
use crate::install::install_resolved_plugin;
use crate::lockfile::{InstallMethod, InstallSource, Lockfile};
use crate::resolve::{Platform, PluginSpecifier, RequestedStrategy, SpecifierSource};
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

    let gh = GitHubClient::new(owner, repo)?;
    let release = gh
        .latest_release(malbox_installer::Channel::Nightly)
        .await?;
    let latest_version = release.version().to_string();

    if !force && !needs_update(&locked.version, &latest_version) {
        return Ok(UpdateOutcome {
            name: name.to_string(),
            old_version: locked.version.clone(),
            new_version: latest_version,
            updated: false,
        });
    }

    let strategy = match locked.install_method {
        InstallMethod::Prebuilt => RequestedStrategy::PrebuiltOnly,
        InstallMethod::Source => RequestedStrategy::SourceOnly,
    };

    let specifier = PluginSpecifier {
        name: name.to_string(),
        version: None,
        source: match locked.source {
            InstallSource::Registry => SpecifierSource::Registry,
            InstallSource::Direct => SpecifierSource::Direct {
                owner: owner.to_string(),
                repo: repo.to_string(),
            },
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
