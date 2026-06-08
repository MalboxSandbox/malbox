use crate::error::{RegistryError, Result};
use crate::index::PluginMetadata;
use crate::lockfile::InstallSource;
use malbox_installer::github::{Release, ReleaseAsset};

#[derive(Debug, Clone)]
pub struct PluginSpecifier {
    pub name: String,
    pub version: Option<String>,
    pub source: SpecifierSource,
}

#[derive(Debug, Clone)]
pub enum SpecifierSource {
    Registry,
    Direct { owner: String, repo: String },
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
}

#[derive(Debug)]
pub enum InstallStrategy {
    Prebuilt {
        release: Release,
        asset: ReleaseAsset,
    },
    Source {
        clone_url: String,
        git_ref: String,
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

    Ok(PluginSpecifier {
        name,
        version,
        source,
    })
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
