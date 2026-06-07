use crate::commands::Command;
use crate::utils::install_progress::CliProgress;
use crate::utils::wizard;
use clap::Parser;
use console::style;
use dialoguer::{Confirm, Input, Select};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::Brand;
use malbox_cli_common::utils::progress::Spinner;
use malbox_installer::config::{InstallConfig, PostgresStrategy};
use malbox_installer::features::{default_features, split_features};
use malbox_installer::github::{Channel, GitHubClient};
use malbox_installer::manifest::Manifest;
use malbox_installer::steps::postgres::{
    detect_postgres_tools, detect_setup_tools, test_connection,
};
use malbox_installer::{GITHUB_OWNER, GITHUB_REPO};

#[derive(Parser)]
#[command(about = "Install and set up Malbox")]
pub struct InstallCommand {
    /// Skip prompts and install with defaults (nightly channel, default
    /// features, prebuilt binaries when available, managed PostgreSQL,
    /// systemd unit)
    #[arg(long)]
    yes: bool,
}

impl Command for InstallCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let header = Brand::accent().bold();
        println!("{}", header.apply_to("Malbox Installation"));
        println!();

        let manifest_path = Manifest::default_path();
        if manifest_path.exists()
            && let Ok(existing) = Manifest::load(&manifest_path)
        {
            match existing.last_completed_step {
                None => {
                    if self.yes {
                        println!("Reinstalling over the existing installation.");
                    } else if !Confirm::new()
                        .with_prompt("Malbox is already installed. Reinstall?")
                        .default(false)
                        .interact()?
                    {
                        println!("Installation cancelled.");
                        return Ok(());
                    }
                }
                Some(last_completed) => {
                    println!(
                        "{}",
                        Brand::warning().apply_to(format!(
                            "Found an incomplete installation of v{} (stopped after the {} step).",
                            existing.version, last_completed
                        ))
                    );
                    let resume = self.yes
                        || Select::new()
                            .with_prompt("Resume it or start fresh?")
                            .items(["Resume installation", "Start fresh"])
                            .default(0)
                            .interact()?
                            == 0;
                    if resume {
                        return resume_install(&manifest_path).await;
                    }
                }
            }
        }

        // Release channel
        let channel = if self.yes {
            Channel::Nightly
        } else {
            wizard::prompt_channel(Channel::Stable)?
        };

        // Daemon features (providers + provisioners)
        let selected_features: Vec<String> = if self.yes {
            default_features()
        } else {
            wizard::prompt_features(&default_features())?
        };

        // Providers configure the daemon; provisioners only affect the build.
        let (providers, provisioners) = split_features(&selected_features);

        // Fetch latest release
        println!();
        let github = GitHubClient::new(GITHUB_OWNER, GITHUB_REPO)
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;
        let release_spinner = Spinner::start("Fetching latest release info");
        let release = github
            .latest_release(channel)
            .await
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;
        drop(release_spinner);

        // Daemon source
        let daemon_source = wizard::resolve_daemon_source(&release, &selected_features, self.yes)?;

        // Frontend - download the prebuilt SPA bundle when the release ships
        // one, otherwise build it from source.
        let frontend_source = wizard::resolve_frontend_source(&release);

        // Postgres
        let postgres = self.select_postgres().await?;

        // Systemd
        let systemd = self.yes
            || Confirm::new()
                .with_prompt("Set up Malbox as a systemd user service?")
                .default(true)
                .interact()?;

        let install_config = InstallConfig {
            providers,
            provisioners,
            features: selected_features,
            channel,
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
            &release,
            &manifest_path,
            &progress,
        )
        .await
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        print_summary(&manifest, &manifest_path);
        Ok(())
    }
}

impl InstallCommand {
    async fn select_postgres(&self) -> Result<PostgresStrategy> {
        if self.yes {
            // Fail here, before the download steps, when the managed
            // instance cannot possibly be set up.
            if !detect_setup_tools() {
                return Err(CliError::CommandFailed(
                    "--yes sets up a managed PostgreSQL instance, but PostgreSQL is not installed \
                     (initdb/psql not found) - install PostgreSQL first, or run interactively to \
                     supply an existing connection URL"
                        .to_string(),
                ));
            }
            return Ok(PostgresStrategy::Setup);
        }

        let strategy = if detect_postgres_tools() {
            let choice = Select::new()
                .with_prompt(
                    "PostgreSQL client tools detected. Use an existing server or set up a new instance?",
                )
                .items([
                    "Use existing (enter connection URL)",
                    "Set up new PostgreSQL instance",
                ])
                .default(0)
                .interact()?;
            match choice {
                0 => PostgresStrategy::Existing {
                    url: prompt_postgres_url(Some("postgres://postgres@localhost:5432/malbox_db"))
                        .await?,
                },
                _ => {
                    ensure_setup_tools()?;
                    PostgresStrategy::Setup
                }
            }
        } else {
            let choice = Select::new()
                .with_prompt("PostgreSQL not found")
                .items([
                    "Set up new PostgreSQL instance",
                    "I'll set it up myself (enter connection URL)",
                ])
                .default(0)
                .interact()?;
            match choice {
                0 => {
                    ensure_setup_tools()?;
                    PostgresStrategy::Setup
                }
                _ => PostgresStrategy::Existing {
                    url: prompt_postgres_url(None).await?,
                },
            }
        };

        Ok(strategy)
    }
}

/// Complete an interrupted installation from where it stopped.
async fn resume_install(manifest_path: &std::path::Path) -> Result<()> {
    let github = GitHubClient::new(GITHUB_OWNER, GITHUB_REPO)
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

    println!();
    println!(
        "{}",
        Brand::accent().bold().apply_to("Resuming installation...")
    );
    println!();

    let progress = CliProgress::new();
    let manifest = malbox_installer::install::resume(&github, manifest_path, &progress)
        .await
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

    print_summary(&manifest, manifest_path);
    Ok(())
}

/// Fail at prompt time when the managed-instance tools are missing, instead
/// of after the download steps.
fn ensure_setup_tools() -> Result<()> {
    if detect_setup_tools() {
        Ok(())
    } else {
        Err(CliError::CommandFailed(
            "setting up a managed PostgreSQL instance requires initdb and psql - install \
             PostgreSQL via your system package manager first, then re-run `malboxctl install`"
                .to_string(),
        ))
    }
}

/// Prompt for a PostgreSQL URL and validate it immediately, so a typo
/// surfaces here instead of minutes later after the download steps.
async fn prompt_postgres_url(default: Option<&str>) -> Result<String> {
    loop {
        // Scoped so the (non-Send) dialoguer builder is dropped before any
        // await point.
        let url: String = {
            let mut input = Input::new().with_prompt("PostgreSQL connection URL");
            if let Some(default) = default {
                input = input.default(default.to_string());
            }
            input.interact_text()?
        };

        let spinner = Spinner::start("Testing connection");
        let outcome = test_connection(&url).await;
        drop(spinner);

        match outcome {
            Ok(()) => return Ok(url),
            Err(e) => {
                println!("  {} {e}", Brand::warning().apply_to("!"));
                if Confirm::new()
                    .with_prompt("Use this URL anyway?")
                    .default(false)
                    .interact()?
                {
                    return Ok(url);
                }
            }
        }
    }
}

fn print_summary(manifest: &Manifest, manifest_path: &std::path::Path) {
    println!();
    println!(
        "{}",
        Brand::success().bold().apply_to(format!(
            "Malbox v{} installed successfully!",
            manifest.version
        ))
    );
    println!();
    println!("  malboxctl: {}", manifest.daemon.path.display());
    println!("  malbox:    {}", manifest.cli.path.display());
    println!("  Frontend:  {}", manifest.frontend.path.display());
    println!("  Config:    ~/.config/malbox/malbox.toml");
    println!("  Manifest:  {}", manifest_path.display());
    println!();

    if manifest.systemd.enabled {
        println!(
            "  Start the daemon with: {}",
            style("systemctl --user start malbox").bold()
        );
        if linger_enabled() == Some(false) {
            println!(
                "  {} user services stop at logout - keep malbox running with: {}",
                Brand::warning().apply_to("!"),
                style("loginctl enable-linger").bold()
            );
        }
    } else {
        println!(
            "  Start the daemon with: {}",
            style("malboxctl daemon start").bold()
        );
        if manifest.postgres.strategy == "setup" {
            println!(
                "  {} the managed PostgreSQL instance was started for this session only;\n    restart it after a reboot with: {}",
                Brand::warning().apply_to("!"),
                style("pg_ctl -D ~/.local/share/malbox/pgdata start").bold()
            );
        }
    }
}

/// Whether systemd lingering is enabled for the current user, i.e. whether
/// user services keep running without an active session.
fn linger_enabled() -> Option<bool> {
    let user = std::env::var("USER").ok()?;
    let output = std::process::Command::new("loginctl")
        .args(["show-user", &user, "--property=Linger"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim() == "Linger=yes")
}
