//! Interactive installation prompts shared by `install` and
//! `upgrade --reconfigure`.

use dialoguer::{MultiSelect, Select};
use malbox_cli_common::error::Result;
use malbox_cli_common::utils::format::{Brand, malbox_theme};
use malbox_installer::config::{DaemonSource, FrontendSource};
use malbox_installer::features::{self, FeatureKind, default_features};
use malbox_installer::github::{Channel, Release};
use malbox_installer::steps::daemon::release_arch;

/// The resolved daemon installation strategy and selected features.
pub struct DaemonChoice {
    pub features: Vec<String>,
    pub source: DaemonSource,
}

pub fn prompt_channel(current: Channel) -> Result<Channel> {
    let default_idx = match current {
        Channel::Stable => 0,
        Channel::Nightly => 1,
    };
    let theme = malbox_theme();
    let choice = Select::with_theme(&theme)
        .with_prompt("Release channel")
        .items(["stable", "nightly"])
        .default(default_idx)
        .interact()?;
    Ok(match choice {
        0 => Channel::Stable,
        _ => Channel::Nightly,
    })
}

/// Default choice for `--yes`: prebuilt binary with all default features.
pub fn default_daemon_choice(release: &Release) -> DaemonChoice {
    let source = release_arch()
        .and_then(|arch| release.find_daemon_asset(arch))
        .map(|asset| DaemonSource::Prebuilt {
            url: asset.browser_download_url.clone(),
        })
        .unwrap_or(DaemonSource::Compile);
    DaemonChoice {
        features: default_features(),
        source,
    }
}

/// Ask the user: prebuilt binary (all features) or compile from source
/// (choose specific providers and provisioners).
pub fn prompt_daemon_choice(release: &Release, current: &[String]) -> Result<DaemonChoice> {
    let prebuilt_asset = release_arch().and_then(|arch| release.find_daemon_asset(arch));

    match prebuilt_asset {
        Some(asset) => {
            let theme = malbox_theme();
            let choice = Select::with_theme(&theme)
                .with_prompt("Daemon source")
                .items(["Download prebuilt binary", "Compile from source"])
                .default(0)
                .interact()?;

            match choice {
                0 => Ok(DaemonChoice {
                    features: default_features(),
                    source: DaemonSource::Prebuilt {
                        url: asset.browser_download_url.clone(),
                    },
                }),
                _ => Ok(DaemonChoice {
                    features: prompt_custom_features(current)?,
                    source: DaemonSource::Compile,
                }),
            }
        }
        None => {
            println!(
                "  {} No prebuilt binary available for this platform - compiling from source",
                Brand::warning().apply_to("!")
            );
            Ok(DaemonChoice {
                features: prompt_custom_features(current)?,
                source: DaemonSource::Compile,
            })
        }
    }
}

/// Categorized multi-select: providers first, then provisioners.
fn prompt_custom_features(current: &[String]) -> Result<Vec<String>> {
    let mut selected = Vec::new();
    let theme = malbox_theme();

    for (kind, label) in [
        (FeatureKind::Provider, "Virtualization providers"),
        (FeatureKind::Provisioner, "Machine provisioners"),
    ] {
        let entries: Vec<&features::DaemonFeature> = features::by_kind(kind).collect();
        if entries.is_empty() {
            continue;
        }

        let display: Vec<&str> = entries.iter().map(|f| f.display).collect();
        let defaults: Vec<bool> = entries
            .iter()
            .map(|f| current.iter().any(|c| c == f.feature))
            .collect();

        let indices = MultiSelect::with_theme(&theme)
            .with_prompt(label)
            .items(&display)
            .defaults(&defaults)
            .interact()?;

        selected.extend(indices.into_iter().map(|i| entries[i].feature.to_string()));
    }

    Ok(selected)
}

/// Front-end source: the prebuilt SPA bundle when the release ships one,
/// otherwise build from source.
pub fn resolve_frontend_source(release: &Release) -> FrontendSource {
    match release.find_web_asset() {
        Some(asset) => FrontendSource::Prebuilt {
            url: asset.browser_download_url.clone(),
        },
        None => FrontendSource::Compile,
    }
}
