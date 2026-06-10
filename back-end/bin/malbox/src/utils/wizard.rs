//! Interactive installation prompts shared by `install` and
//! `upgrade --reconfigure`.

use dialoguer::{MultiSelect, Select};
use malbox_cli_common::error::Result;
use malbox_cli_common::utils::format::Brand;
use malbox_installer::config::{DaemonSource, FrontendSource};
use malbox_installer::features::{DAEMON_FEATURES, default_features};
use malbox_installer::github::{Channel, Release};
use malbox_installer::steps::daemon::release_arch;

pub fn prompt_channel(current: Channel) -> Result<Channel> {
    let default_idx = match current {
        Channel::Stable => 0,
        Channel::Nightly => 1,
    };
    let choice = Select::new()
        .with_prompt("Release channel")
        .items(["stable", "nightly"])
        .default(default_idx)
        .interact()?;
    Ok(match choice {
        0 => Channel::Stable,
        _ => Channel::Nightly,
    })
}

/// Multi-select over the known daemon features, pre-selecting `current`.
pub fn prompt_features(current: &[String]) -> Result<Vec<String>> {
    let display_names: Vec<&str> = DAEMON_FEATURES.iter().map(|f| f.display).collect();
    let defaults: Vec<bool> = DAEMON_FEATURES
        .iter()
        .map(|f| current.iter().any(|c| c == f.feature))
        .collect();

    Ok(MultiSelect::new()
        .with_prompt("Select daemon features to include")
        .items(&display_names)
        .defaults(&defaults)
        .interact()?
        .into_iter()
        .map(|i| DAEMON_FEATURES[i].feature.to_string())
        .collect())
}

/// Decide the daemon binary source. Prebuilt is only offered when the
/// selection is exactly the feature set the release binaries were built
/// with; `assume_yes` takes the prebuilt without prompting.
pub fn resolve_daemon_source(
    release: &Release,
    selected_features: &[String],
    assume_yes: bool,
) -> Result<DaemonSource> {
    let matches_defaults = {
        let mut selected = selected_features.to_vec();
        selected.sort_unstable();
        let mut defaults = default_features();
        defaults.sort_unstable();
        selected == defaults
    };
    let prebuilt_asset = matches_defaults
        .then(release_arch)
        .flatten()
        .and_then(|arch| release.find_daemon_asset(arch));

    let source = match prebuilt_asset {
        Some(asset) => {
            let use_prebuilt = assume_yes
                || Select::new()
                    .with_prompt("A prebuilt binary is available for the default feature set")
                    .items(["Download prebuilt binary (faster)", "Compile from source"])
                    .default(0)
                    .interact()?
                    == 0;
            if use_prebuilt {
                DaemonSource::Prebuilt {
                    url: asset.browser_download_url.clone(),
                }
            } else {
                DaemonSource::Compile
            }
        }
        None => {
            if !matches_defaults {
                println!(
                    "  {} Custom feature selection requires compiling from source",
                    Brand::warning().apply_to("!")
                );
            } else {
                println!(
                    "  {} No prebuilt binary available for this platform - will compile from source",
                    Brand::warning().apply_to("!")
                );
            }
            DaemonSource::Compile
        }
    };
    Ok(source)
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
