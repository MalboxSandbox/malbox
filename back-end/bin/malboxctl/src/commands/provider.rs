//! Provider management commands.
//!
//! Manages which provider crates are compiled into the malbox daemon.
//! Providers are toggled in the daemon config's `[providers]` section and
//! compiled in by rebuilding the installed binaries (Cargo features) from
//! the installed version's source.

use crate::commands::Command;
use crate::utils::install_progress::CliProgress;
use clap::{Parser, Subcommand};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::Brand;
use malbox_installer::features::{default_features, provider_feature, split_features};
use malbox_installer::github::GitHubClient;
use malbox_installer::manifest::Manifest;
use malbox_installer::{GITHUB_OWNER, GITHUB_REPO};

pub mod install;
pub mod list;
pub mod rebuild;
pub mod uninstall;

pub use install::InstallCommand;
pub use list::ListCommand;
pub use rebuild::RebuildCommand;
pub use uninstall::UninstallCommand;

#[derive(Parser)]
#[command(about = "Manage virtualization providers")]
pub struct ProviderCommand {
    #[command(subcommand)]
    command: ProviderCommands,
}

#[derive(Subcommand)]
enum ProviderCommands {
    /// List providers compiled into daemon
    List(ListCommand),
    /// Install a provider
    Install(InstallCommand),
    /// Uninstall a provider
    Uninstall(UninstallCommand),
    /// Rebuild daemon with current provider configuration
    Rebuild(RebuildCommand),
}

impl Command for ProviderCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            ProviderCommands::List(cmd) => cmd.execute(ctx).await,
            ProviderCommands::Install(cmd) => cmd.execute(ctx).await,
            ProviderCommands::Uninstall(cmd) => cmd.execute(ctx).await,
            ProviderCommands::Rebuild(cmd) => cmd.execute(ctx).await,
        }
    }
}

/// Validate a provider name against the features the build knows about.
pub(crate) fn ensure_known_provider(name: &str) -> Result<&'static str> {
    provider_feature(name).ok_or_else(|| {
        let known: Vec<&str> = malbox_installer::features::known_providers().collect();
        CliError::CommandFailed(format!(
            "unknown provider '{name}' - known providers: {}",
            known.join(", ")
        ))
    })
}

/// Cargo features the installed binaries should carry for `providers`,
/// keeping the provisioner choices recorded in the manifest.
pub(crate) fn desired_features(manifest: &Manifest, providers: &[String]) -> Result<Vec<String>> {
    let mut features = Vec::new();
    for provider in providers {
        features.push(ensure_known_provider(provider)?.to_string());
    }
    let provisioners = if manifest.daemon.provisioners.is_empty() {
        split_features(&default_features()).1
    } else {
        manifest.daemon.provisioners.clone()
    };
    features.extend(
        provisioners
            .iter()
            .map(|provisioner| format!("provisioner-{provisioner}")),
    );
    Ok(features)
}

/// True when the installed binaries already carry exactly `features`.
pub(crate) fn binary_matches(manifest: &Manifest, features: &[String]) -> bool {
    let mut have = manifest.daemon.features.clone();
    have.sort_unstable();
    let mut want = features.to_vec();
    want.sort_unstable();
    have == want
}

/// Recompile the installed binaries with `features` and swap them in.
pub(crate) async fn rebuild_daemon(features: &[String]) -> Result<()> {
    let manifest_path = Manifest::default_path();
    let manifest =
        Manifest::load(&manifest_path).map_err(|e| CliError::CommandFailed(e.to_string()))?;

    println!();
    println!(
        "{}",
        Brand::accent().bold().apply_to(format!(
            "Rebuilding malbox v{} with features: {}",
            manifest.version,
            features.join(", ")
        ))
    );
    println!();

    let github = GitHubClient::new(GITHUB_OWNER, GITHUB_REPO)
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;
    let progress = CliProgress::new();
    let manifest = malbox_installer::rebuild::run(features, &github, &manifest_path, &progress)
        .await
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

    println!();
    println!(
        "{}",
        Brand::success().bold().apply_to(format!(
            "Daemon rebuilt (providers: {})",
            if manifest.daemon.providers.is_empty() {
                "none".to_string()
            } else {
                manifest.daemon.providers.join(", ")
            }
        ))
    );
    Ok(())
}
