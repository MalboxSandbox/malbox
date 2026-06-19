use crate::commands::Command;
use crate::utils::install_renderer::InstallRenderer;
use crate::utils::wizard;
use clap::Parser;
use console::style;
use dialoguer::{Confirm, Input, Select};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::{self, Brand};
use malbox_cli_common::utils::progress::Spinner;
use malbox_installer::config::{InstallConfig, PostgresStrategy};
use malbox_installer::features::split_features;
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
        let accent = Brand::accent();
        for line in [
            r"            _ _",
            r" _ __  __ _| | |__  _____ __",
            r"| '  \/ _` | | '_ \/ _ \ \ /",
            r"|_|_|_\__,_|_|_.__/\___/_\_\",
        ] {
            println!("  {}", accent.apply_to(line));
        }
        println!();

        let manifest_path = Manifest::default_path();
        if manifest_path.exists()
            && let Ok(existing) = Manifest::load(&manifest_path)
        {
            let theme = format::malbox_theme();
            match existing.last_completed_step {
                None => {
                    if self.yes {
                        println!("  Reinstalling over existing installation.");
                    } else if !Confirm::with_theme(&theme)
                        .with_prompt("Malbox is already installed. Reinstall?")
                        .default(false)
                        .interact()?
                    {
                        println!("  Installation cancelled.");
                        return Ok(());
                    }
                }
                Some(last_completed) => {
                    println!(
                        "  {} Incomplete installation of v{} (stopped after {} step)",
                        Brand::warning().apply_to("!"),
                        existing.version,
                        last_completed
                    );
                    let resume = self.yes
                        || Select::with_theme(&theme)
                            .with_prompt("Resume or start fresh?")
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

        if !self.yes {
            format::section_divider("Configuration");
        }

        let channel = if self.yes {
            Channel::Nightly
        } else {
            wizard::prompt_channel(Channel::Stable)?
        };

        let github = GitHubClient::new(GITHUB_OWNER, GITHUB_REPO)
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;
        let release_spinner = Spinner::start("Fetching latest release info");
        let release = github
            .latest_release(channel)
            .await
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;
        drop(release_spinner);

        let daemon_choice = if self.yes {
            wizard::default_daemon_choice(&release)
        } else {
            wizard::prompt_daemon_choice(&release, &malbox_installer::default_features())?
        };
        let (providers, provisioners) = split_features(&daemon_choice.features);

        let frontend_source = wizard::resolve_frontend_source(&release);

        let postgres = self.select_postgres().await?;

        let theme = format::malbox_theme();
        let systemd = self.yes
            || Confirm::with_theme(&theme)
                .with_prompt("Enable systemd user service?")
                .default(true)
                .interact()?;

        let install_config = InstallConfig {
            providers,
            provisioners,
            features: daemon_choice.features,
            channel,
            daemon: daemon_choice.source,
            frontend: frontend_source,
            postgres,
            systemd,
        };

        // Run installation
        format::section_divider("Installation");

        let renderer = InstallRenderer::new("Installing Malbox");
        renderer.add_pending_steps(&[
            "Installing malbox binaries",
            "Installing front-end assets",
            "Setting up PostgreSQL",
            "Generating default configuration",
            "Configuring systemd user services",
        ]);
        renderer.add_recovery_hints(
            "Installing malbox binaries",
            &["Re-run `malbox daemon install` to resume from this step"],
        );
        renderer.add_recovery_hints(
            "Installing front-end assets",
            &["Re-run `malbox daemon install` to resume from this step"],
        );
        renderer.add_recovery_hints(
            "Setting up PostgreSQL",
            &[
                "Ensure PostgreSQL tools are installed: psql --version",
                "Re-run `malbox daemon install` to resume from this step",
            ],
        );
        renderer.add_recovery_hints(
            "Generating default configuration",
            &["Re-run `malbox daemon install` to resume from this step"],
        );
        renderer.add_recovery_hints(
            "Configuring systemd user services",
            &[
                "Verify systemd user session: systemctl --user status",
                "Re-run `malbox daemon install` to resume from this step",
            ],
        );

        let manifest = malbox_installer::install::run(
            &install_config,
            &github,
            &release,
            &manifest_path,
            &renderer,
        )
        .await
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        renderer.finish_line(&format!("Malbox v{} installed", manifest.version));
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

        let theme = format::malbox_theme();
        let strategy = if detect_postgres_tools() {
            let choice = Select::with_theme(&theme)
                .with_prompt("PostgreSQL setup")
                .items(["Use existing server", "Set up new instance"])
                .default(0)
                .interact()?;
            match choice {
                0 => PostgresStrategy::Existing {
                    url: prompt_postgres_url(Some("postgres://postgres@localhost:5432")).await?,
                },
                _ => {
                    ensure_setup_tools()?;
                    PostgresStrategy::Setup
                }
            }
        } else {
            println!(
                "  {} PostgreSQL client tools not found",
                Brand::warning().apply_to("!")
            );
            let choice = Select::with_theme(&theme)
                .with_prompt("PostgreSQL setup")
                .items(["Set up new instance", "Enter connection URL"])
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

    let renderer = InstallRenderer::new("Resuming installation");
    renderer.add_pending_steps(&[
        "Installing malbox binaries",
        "Installing front-end assets",
        "Setting up PostgreSQL",
        "Generating default configuration",
        "Configuring systemd user services",
    ]);
    renderer.add_recovery_hints(
        "Installing malbox binaries",
        &["Re-run `malbox daemon install` to retry from this step"],
    );
    renderer.add_recovery_hints(
        "Installing front-end assets",
        &["Re-run `malbox daemon install` to retry from this step"],
    );
    renderer.add_recovery_hints(
        "Setting up PostgreSQL",
        &[
            "Ensure PostgreSQL tools are installed: psql --version",
            "Re-run `malbox daemon install` to retry from this step",
        ],
    );
    renderer.add_recovery_hints(
        "Generating default configuration",
        &["Re-run `malbox daemon install` to retry from this step"],
    );
    renderer.add_recovery_hints(
        "Configuring systemd user services",
        &[
            "Verify systemd user session: systemctl --user status",
            "Re-run `malbox daemon install` to retry from this step",
        ],
    );

    let manifest = malbox_installer::install::resume(&github, manifest_path, &renderer)
        .await
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

    renderer.finish_line(&format!("Malbox v{} installed", manifest.version));
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
             PostgreSQL via your system package manager first, then re-run `malbox daemon install`"
                .to_string(),
        ))
    }
}

/// Prompt for a PostgreSQL URL and validate it immediately, so a typo
/// surfaces here instead of minutes later after the download steps.
async fn prompt_postgres_url(default: Option<&str>) -> Result<String> {
    loop {
        let url: String = {
            let theme = format::malbox_theme();
            let mut input = Input::with_theme(&theme).with_prompt("Connection URL");
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
                let theme = format::malbox_theme();
                if Confirm::with_theme(&theme)
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
    let dim = Brand::dim();

    println!();
    let paths: &[(&str, String)] = &[
        ("malboxd", manifest.daemon.path.display().to_string()),
        ("malbox", manifest.cli.path.display().to_string()),
        ("front-end", manifest.frontend.path.display().to_string()),
        ("config", "~/.config/malbox/malbox.toml".to_string()),
        ("manifest", manifest_path.display().to_string()),
    ];
    let max_label = paths.iter().map(|(l, _)| l.len()).max().unwrap_or(0);
    for (label, path) in paths {
        println!(
            "  {}  {}",
            dim.apply_to(format!("{:<width$}", label, width = max_label)),
            path,
        );
    }

    println!();
    let start_cmd = if manifest.systemd.enabled {
        "systemctl --user start malbox"
    } else {
        "malbox daemon start"
    };
    println!(
        "  {} {}",
        dim.apply_to("Start the daemon with:"),
        style(start_cmd).bold(),
    );

    println!(
        "  {}",
        dim.apply_to("Documentation: https://docs.malbox.app"),
    );

    if manifest.systemd.enabled && linger_enabled() == Some(false) {
        println!(
            "  {} {}",
            Brand::warning().apply_to("!"),
            dim.apply_to(format!(
                "User services stop at logout - enable linger: {}",
                style("loginctl enable-linger").bold()
            )),
        );
    }
    if !manifest.systemd.enabled && manifest.postgres.strategy == "setup" {
        println!(
            "  {} {}",
            Brand::warning().apply_to("!"),
            dim.apply_to(format!(
                "Managed PostgreSQL started for this session only - after reboot: {}",
                style("pg_ctl -D ~/.local/share/malbox/pgdata start").bold()
            )),
        );
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
