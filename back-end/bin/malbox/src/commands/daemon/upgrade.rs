use crate::commands::Command;
use crate::utils::install_progress::CliProgress;
use crate::utils::provider_config::ProvidersEdit;
use crate::utils::wizard;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::Brand;
use malbox_cli_common::utils::progress::Spinner;
use malbox_installer::config::{InstallConfig, PostgresStrategy, UpgradeConfig};
use malbox_installer::error::InstallError;
use malbox_installer::features::{default_features, split_features};
use malbox_installer::github::GitHubClient;
use malbox_installer::manifest::Manifest;

const GITHUB_OWNER: &str = malbox_installer::GITHUB_OWNER;
const GITHUB_REPO: &str = malbox_installer::GITHUB_REPO;

#[derive(Parser)]
#[command(about = "Upgrade Malbox to the latest version")]
pub struct UpgradeCommand {
    /// Force reinstall even if already up to date
    #[arg(long)]
    force: bool,

    /// Re-prompt the build-related choices (channel, daemon features,
    /// binary source) and rebuild with them
    #[arg(long)]
    reconfigure: bool,

    /// Roll back to the previous version
    #[arg(long)]
    rollback: bool,
}

impl Command for UpgradeCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let manifest_path = Manifest::default_path();

        if self.rollback {
            let header = Brand::accent().bold();
            println!("{}", header.apply_to("Rolling back Malbox..."));
            println!();

            let progress = CliProgress::new();
            malbox_installer::upgrade::rollback(&manifest_path, &progress)
                .await
                .map_err(|e| CliError::CommandFailed(e.to_string()))?;

            println!();
            println!("{}", Brand::success().bold().apply_to("Rollback complete."));
            return Ok(());
        }

        let header = Brand::accent().bold();
        println!(
            "{}",
            header.apply_to(if self.reconfigure {
                "Reconfiguring Malbox..."
            } else {
                "Upgrading Malbox..."
            })
        );
        println!();

        let github = GitHubClient::new(GITHUB_OWNER, GITHUB_REPO)
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let reconfigure = if self.reconfigure {
            Some(prompt_reconfigure(&github, &manifest_path).await?)
        } else {
            None
        };

        let upgrade_config = UpgradeConfig {
            force: self.force,
            reconfigure,
        };

        let progress = CliProgress::new();

        match malbox_installer::upgrade::run(&upgrade_config, &github, &manifest_path, &progress)
            .await
        {
            Ok(manifest) => {
                // Keep the daemon config's provider list in sync with the
                // reconfigured feature set; the daemon refuses to start on a
                // mismatch.
                if let Some(reconfig) = &upgrade_config.reconfigure
                    && let Err(e) = sync_providers(&reconfig.providers)
                {
                    println!(
                        "  {} could not update providers in the daemon config: {e}\n    Set [providers] enabled = {:?} manually.",
                        Brand::warning().apply_to("!"),
                        reconfig.providers
                    );
                }
                println!();
                println!(
                    "{}",
                    Brand::success()
                        .bold()
                        .apply_to(format!("Malbox upgraded to v{}!", manifest.version))
                );
            }
            Err(InstallError::AlreadyUpToDate(version)) => {
                println!(
                    "  {} Already up to date (v{version})",
                    Brand::success().apply_to("\u{2713}")
                );
            }
            Err(InstallError::ManifestNotFound(_)) => {
                eprintln!(
                    "  {} Malbox is not installed. Run {} first.",
                    Brand::error().apply_to("\u{2717}"),
                    console::style("malbox daemon install").bold()
                );
            }
            Err(e) => {
                return Err(CliError::CommandFailed(e.to_string()));
            }
        }

        Ok(())
    }
}

/// Re-prompt the build-related installation choices. Postgres and systemd
/// are infrastructure choices an upgrade never re-provisions; they carry
/// over from the manifest.
async fn prompt_reconfigure(
    github: &GitHubClient,
    manifest_path: &std::path::Path,
) -> Result<InstallConfig> {
    let manifest =
        Manifest::load(manifest_path).map_err(|e| CliError::CommandFailed(e.to_string()))?;

    let channel = wizard::prompt_channel(manifest.channel)?;

    let current_features = if manifest.daemon.features.is_empty() {
        default_features()
    } else {
        manifest.daemon.features.clone()
    };
    let selected_features = wizard::prompt_features(&current_features)?;
    let (providers, provisioners) = split_features(&selected_features);

    let spinner = Spinner::start("Fetching latest release info");
    let release = github
        .latest_release(channel)
        .await
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;
    drop(spinner);

    let daemon = wizard::resolve_daemon_source(&release, &selected_features, false)?;
    let frontend = wizard::resolve_frontend_source(&release);

    Ok(InstallConfig {
        providers,
        provisioners,
        features: selected_features,
        channel,
        daemon,
        frontend,
        postgres: PostgresStrategy::Existing {
            url: manifest.postgres.url.clone(),
        },
        systemd: manifest.systemd.enabled,
    })
}

/// Point the daemon config's `[providers]` section at the reconfigured set.
fn sync_providers(providers: &[String]) -> Result<()> {
    let mut edit = ProvidersEdit::load()?;
    edit.set_enabled(providers);
    edit.save()?;
    println!(
        "  {} providers.enabled updated in {}",
        Brand::success().apply_to("\u{2713}"),
        edit.path().display()
    );
    Ok(())
}
