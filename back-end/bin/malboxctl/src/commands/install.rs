use crate::commands::Command;
use crate::utils::install_progress::CliProgress;
use clap::Parser;
use console::style;
use dialoguer::{Confirm, MultiSelect, Select};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_cli_common::utils::format::Brand;
use malbox_cli_common::utils::progress::Spinner;
use malbox_installer::config::{
    DaemonSource, FrontendSource, InstallConfig, NixStrategy, PostgresStrategy,
};
use malbox_installer::github::GitHubClient;
use malbox_installer::manifest::Manifest;

const GITHUB_OWNER: &str = "malboxapp";
const GITHUB_REPO: &str = "malbox";

const AVAILABLE_PROVIDERS: &[&str] = &["libvirt", "ansible"];

#[derive(Parser)]
#[command(about = "Install and set up Malbox")]
pub struct InstallCommand {
    /// Skip confirmation prompts
    #[arg(long)]
    yes: bool,
}

impl Command for InstallCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let header = Brand::accent().bold();
        println!("{}", header.apply_to("Malbox Installation"));
        println!();

        let manifest_path = Manifest::default_path();
        if manifest_path.exists() {
            let existing = Manifest::load(&manifest_path);
            if let Ok(m) = existing {
                if m.last_completed_step.is_none() {
                    if !Confirm::new()
                        .with_prompt("Malbox is already installed. Reinstall?")
                        .default(false)
                        .interact()?
                    {
                        println!("Installation cancelled.");
                        return Ok(());
                    }
                } else {
                    println!(
                        "{}",
                        Brand::warning().apply_to(
                            "Previous installation was incomplete. Resuming is not yet supported - starting fresh."
                        )
                    );
                }
            }
        }

        // Nix
        let nix_strategy = if malbox_installer::steps::nix::detect_nix() {
            println!("  {} Nix detected", Brand::success().apply_to("\u{2713}"));
            NixStrategy::Existing
        } else {
            let install_nix = Select::new()
                .with_prompt("Nix is not installed. Nix provides reproducible builds and dependency management")
                .items(["Install Nix (recommended)", "Skip Nix"])
                .default(0)
                .interact()?;
            match install_nix {
                0 => NixStrategy::Install,
                _ => NixStrategy::Skip,
            }
        };

        // Providers
        let provider_selections = MultiSelect::new()
            .with_prompt("Select providers to include")
            .items(AVAILABLE_PROVIDERS)
            .defaults(&vec![true; AVAILABLE_PROVIDERS.len()])
            .interact()?;

        let providers: Vec<String> = provider_selections
            .iter()
            .map(|&i| AVAILABLE_PROVIDERS[i].to_string())
            .collect();

        // Fetch latest release to check for prebuilt binary
        println!();
        let github = GitHubClient::new(GITHUB_OWNER, GITHUB_REPO);
        let release_spinner = Spinner::start("Fetching latest release info");
        let release = github
            .latest_release()
            .await
            .map_err(|e| malbox_cli_common::error::CliError::CommandFailed(e.to_string()))?;
        drop(release_spinner);

        let arch = format!(
            "{}-unknown-{}-gnu",
            std::env::consts::ARCH,
            std::env::consts::OS
        );
        let provider_refs: Vec<&str> = providers.iter().map(|s| s.as_str()).collect();
        let has_prebuilt = release.find_daemon_asset(&arch, &provider_refs).is_some();

        // Daemon source
        let daemon_source = if has_prebuilt {
            let choice = Select::new()
                .with_prompt("A prebuilt binary is available for your provider selection")
                .items(["Download prebuilt binary (faster)", "Compile from source"])
                .default(0)
                .interact()?;
            match choice {
                0 => {
                    let asset = release.find_daemon_asset(&arch, &provider_refs).unwrap();
                    DaemonSource::Prebuilt {
                        url: asset.browser_download_url.clone(),
                    }
                }
                _ => DaemonSource::Compile {
                    features: providers.iter().map(|p| format!("provider-{p}")).collect(),
                },
            }
        } else {
            println!(
                "  {} No prebuilt binary for this provider combination - will compile from source",
                Brand::warning().apply_to("!")
            );
            DaemonSource::Compile {
                features: providers.iter().map(|p| format!("provider-{p}")).collect(),
            }
        };

        // Frontend source
        let frontend_source = if release.find_frontend_asset().is_some() {
            let choice = Select::new()
                .with_prompt("Front-end assets")
                .items(["Download prebuilt assets (faster)", "Build from source"])
                .default(0)
                .interact()?;
            match choice {
                0 => {
                    let asset = release.find_frontend_asset().unwrap();
                    FrontendSource::Prebuilt {
                        url: asset.browser_download_url.clone(),
                    }
                }
                _ => FrontendSource::Compile,
            }
        } else {
            FrontendSource::Compile
        };

        // Postgres
        let pg_detected = malbox_installer::steps::postgres::detect_postgres();
        let postgres = if pg_detected {
            let choice = Select::new()
                .with_prompt("PostgreSQL detected. Use existing instance or set up a new one?")
                .items([
                    "Use existing (enter connection URL)",
                    "Set up new PostgreSQL via Nix",
                ])
                .default(0)
                .interact()?;
            match choice {
                0 => {
                    let url: String = dialoguer::Input::new()
                        .with_prompt("PostgreSQL connection URL")
                        .default("postgres://postgres@localhost:5433/malbox_db".to_string())
                        .interact_text()?;
                    PostgresStrategy::Existing { url }
                }
                _ => PostgresStrategy::Setup,
            }
        } else {
            let choice = Select::new()
                .with_prompt("PostgreSQL not found")
                .items([
                    "Install and set up PostgreSQL via Nix",
                    "I'll set it up myself (enter connection URL)",
                ])
                .default(0)
                .interact()?;
            match choice {
                0 => PostgresStrategy::Setup,
                _ => {
                    let url: String = dialoguer::Input::new()
                        .with_prompt("PostgreSQL connection URL")
                        .interact_text()?;
                    PostgresStrategy::Existing { url }
                }
            }
        };

        // Systemd
        let systemd = Confirm::new()
            .with_prompt("Set up Malbox as a systemd user service?")
            .default(true)
            .interact()?;

        let install_config = InstallConfig {
            nix: nix_strategy,
            providers,
            daemon: daemon_source,
            frontend: frontend_source,
            postgres,
            systemd,
        };

        // Run installation
        println!();
        println!("{}", header.apply_to("Installing Malbox..."));
        println!();

        let progress = CliProgress::new();
        let manifest = malbox_installer::install::run(
            &install_config,
            &github,
            &release.tag_name,
            &manifest_path,
            &progress,
        )
        .await
        .map_err(|e| malbox_cli_common::error::CliError::CommandFailed(e.to_string()))?;

        println!();
        println!(
            "{}",
            Brand::success()
                .bold()
                .apply_to("Malbox installed successfully!")
        );
        println!();
        println!("  Daemon:   {}", manifest.daemon.path.display());
        println!("  Frontend: {}", manifest.frontend.path.display());
        println!("  Config:   ~/.config/malbox/malbox.toml");
        println!("  Manifest: {}", manifest_path.display());
        if manifest.systemd.enabled {
            println!();
            println!(
                "  Start the daemon with: {}",
                style("systemctl --user start malbox").bold()
            );
        } else {
            println!();
            println!(
                "  Start the daemon with: {}",
                style("malboxctl daemon start").bold()
            );
        }

        Ok(())
    }
}
