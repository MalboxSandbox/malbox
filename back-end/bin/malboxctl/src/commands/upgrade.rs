use crate::commands::Command;
use crate::utils::install_progress::CliProgress;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_cli_common::utils::format::Brand;
use malbox_installer::config::UpgradeConfig;
use malbox_installer::error::InstallError;
use malbox_installer::github::GitHubClient;
use malbox_installer::manifest::Manifest;

const GITHUB_OWNER: &str = "malboxapp";
const GITHUB_REPO: &str = "malbox";

#[derive(Parser)]
#[command(about = "Upgrade Malbox to the latest version")]
pub struct UpgradeCommand {
    /// Force reinstall even if already up to date
    #[arg(long)]
    force: bool,

    /// Re-prompt for all installation choices
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
                .map_err(|e| malbox_cli_common::error::CliError::CommandFailed(e.to_string()))?;

            println!();
            println!("{}", Brand::success().bold().apply_to("Rollback complete."));
            return Ok(());
        }

        let header = Brand::accent().bold();
        println!("{}", header.apply_to("Upgrading Malbox..."));
        println!();

        let reconfigure = if self.reconfigure {
            println!(
                "{}",
                Brand::warning().apply_to("--reconfigure is not yet implemented")
            );
            None
        } else {
            None
        };

        let upgrade_config = UpgradeConfig {
            force: self.force,
            reconfigure,
        };

        let github = GitHubClient::new(GITHUB_OWNER, GITHUB_REPO);
        let progress = CliProgress::new();

        match malbox_installer::upgrade::run(&upgrade_config, &github, &manifest_path, &progress)
            .await
        {
            Ok(manifest) => {
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
                    console::style("malboxctl install").bold()
                );
            }
            Err(e) => {
                return Err(malbox_cli_common::error::CliError::CommandFailed(
                    e.to_string(),
                ));
            }
        }

        Ok(())
    }
}
