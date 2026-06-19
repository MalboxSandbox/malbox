use crate::commands::Command;
use crate::utils::install_renderer::InstallRenderer;
use crate::utils::provider_config::ProvidersEdit;
use crate::utils::wizard;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::{self, Brand};
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
            let renderer = InstallRenderer::new("Rolling back Malbox");
            renderer.add_pending_steps(&[
                "Rolling back malbox binaries",
                "Rolling back front-end assets",
                "Starting rolled-back daemon",
            ]);
            renderer.add_recovery_hints(
                "Rolling back malbox binaries",
                &["Re-run `malbox daemon upgrade --rollback` to retry"],
            );
            renderer.add_recovery_hints(
                "Rolling back front-end assets",
                &["Re-run `malbox daemon upgrade --rollback` to retry"],
            );
            renderer.add_recovery_hints(
                "Starting rolled-back daemon",
                &["Start manually: systemctl --user start malbox"],
            );

            malbox_installer::upgrade::rollback(&manifest_path, &renderer)
                .await
                .map_err(|e| CliError::CommandFailed(e.to_string()))?;

            renderer.finish_line("Rollback complete");
            return Ok(());
        }

        let github = GitHubClient::new(GITHUB_OWNER, GITHUB_REPO)
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let reconfigure = if self.reconfigure {
            format::section_divider("Configuration");
            Some(prompt_reconfigure(&github, &manifest_path).await?)
        } else {
            None
        };

        let upgrade_config = UpgradeConfig {
            force: self.force,
            reconfigure,
        };

        if self.reconfigure {
            format::section_divider("Reconfiguration");
        }

        let renderer = InstallRenderer::new(if self.reconfigure {
            "Reconfiguring Malbox"
        } else {
            "Upgrading Malbox"
        });
        renderer.add_pending_steps(&[
            "Stopping daemon",
            "Installing malbox binaries",
            "Installing front-end assets",
            "Setting up PostgreSQL",
            "Starting upgraded daemon",
        ]);
        renderer.add_recovery_hints(
            "Stopping daemon",
            &["Force stop: systemctl --user stop malbox"],
        );
        renderer.add_recovery_hints(
            "Installing malbox binaries",
            &["Re-run `malbox daemon upgrade` to retry"],
        );
        renderer.add_recovery_hints(
            "Installing front-end assets",
            &["Re-run `malbox daemon upgrade` to retry"],
        );
        renderer.add_recovery_hints(
            "Setting up PostgreSQL",
            &["Re-run `malbox daemon upgrade` to retry"],
        );
        renderer.add_recovery_hints(
            "Starting upgraded daemon",
            &["Start manually: systemctl --user start malbox"],
        );

        match malbox_installer::upgrade::run(&upgrade_config, &github, &manifest_path, &renderer)
            .await
        {
            Ok(manifest) => {
                if let Some(reconfig) = &upgrade_config.reconfigure
                    && let Err(e) = sync_providers(&reconfig.providers)
                {
                    println!(
                        "  {} could not update providers in the daemon config: {e}\n    Set [providers] enabled = {:?} manually.",
                        Brand::warning().apply_to("!"),
                        reconfig.providers
                    );
                }
                renderer.finish_line(&format!("Malbox upgraded to v{}!", manifest.version));
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

    let spinner = Spinner::start("Fetching latest release info");
    let release = github
        .latest_release(channel)
        .await
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;
    drop(spinner);

    let current_features = if manifest.daemon.features.is_empty() {
        default_features()
    } else {
        manifest.daemon.features.clone()
    };
    let daemon_choice = wizard::prompt_daemon_choice(&release, &current_features)?;
    let (providers, provisioners) = split_features(&daemon_choice.features);
    let frontend = wizard::resolve_frontend_source(&release);

    Ok(InstallConfig {
        providers,
        provisioners,
        features: daemon_choice.features,
        channel,
        daemon: daemon_choice.source,
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
