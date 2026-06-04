use crate::commands::Command;
use crate::utils::install_progress::CliProgress;
use clap::Parser;
use console::style;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::Brand;
use malbox_cli_common::utils::progress::Spinner;
use malbox_installer::config::{DaemonSource, FrontendSource, InstallConfig, PostgresStrategy};
use malbox_installer::github::{Channel, GitHubClient, Release};
use malbox_installer::manifest::Manifest;
use malbox_installer::steps::daemon::release_arch;
use malbox_installer::steps::postgres::{detect_postgres_tools, test_connection};
use malbox_installer::{DEFAULT_DAEMON_FEATURES, GITHUB_OWNER, GITHUB_REPO};

struct DaemonFeature {
    display: &'static str,
    feature: &'static str,
    default: bool,
}

const DAEMON_FEATURES: &[DaemonFeature] = &[
    DaemonFeature {
        display: "libvirt (virtualization provider)",
        feature: "provider-libvirt",
        default: true,
    },
    DaemonFeature {
        display: "xen (virtualization provider)",
        feature: "provider-xen",
        default: false,
    },
    DaemonFeature {
        display: "ansible (machine provisioner)",
        feature: "provisioner-ansible",
        default: true,
    },
];

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
            if existing.last_completed_step.is_none() {
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
            } else {
                println!(
                    "{}",
                    Brand::warning().apply_to(
                        "Previous installation was incomplete. Resuming is not yet supported - starting fresh."
                    )
                );
            }
        }

        // Release channel
        let channel = if self.yes {
            Channel::Nightly
        } else {
            let choice = Select::new()
                .with_prompt("Release channel")
                .items(["stable", "nightly"])
                .default(0)
                .interact()?;
            match choice {
                0 => Channel::Stable,
                _ => Channel::Nightly,
            }
        };

        // Daemon features (providers + provisioners)
        let selected_features: Vec<String> = if self.yes {
            DEFAULT_DAEMON_FEATURES
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            let display_names: Vec<&str> = DAEMON_FEATURES.iter().map(|f| f.display).collect();
            let defaults: Vec<bool> = DAEMON_FEATURES.iter().map(|f| f.default).collect();

            MultiSelect::new()
                .with_prompt("Select daemon features to include")
                .items(&display_names)
                .defaults(&defaults)
                .interact()?
                .into_iter()
                .map(|i| DAEMON_FEATURES[i].feature.to_string())
                .collect()
        };

        // Providers configure the daemon; provisioners only affect the build.
        let providers: Vec<String> = strip_prefixed(&selected_features, "provider-");
        let provisioners: Vec<String> = strip_prefixed(&selected_features, "provisioner-");

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

        let matches_defaults = {
            let mut selected: Vec<&str> = selected_features.iter().map(String::as_str).collect();
            selected.sort_unstable();
            let mut defaults = DEFAULT_DAEMON_FEATURES.to_vec();
            defaults.sort_unstable();
            selected == defaults
        };
        let prebuilt_asset = matches_defaults
            .then(release_arch)
            .flatten()
            .and_then(|arch| release.find_malboxctl_asset(arch));

        // Daemon source
        let daemon_source = match prebuilt_asset {
            Some(asset) => {
                let use_prebuilt = self.yes
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

        // Frontend - download the prebuilt SPA bundle when the release ships
        // one, otherwise build it from source.
        let frontend_source = match release.find_web_asset() {
            Some(asset) => FrontendSource::Prebuilt {
                url: asset.browser_download_url.clone(),
            },
            None => FrontendSource::Compile,
        };

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

        print_summary(&install_config, &manifest, &manifest_path, &release);
        Ok(())
    }
}

impl InstallCommand {
    async fn select_postgres(&self) -> Result<PostgresStrategy> {
        if self.yes {
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
                _ => PostgresStrategy::Setup,
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
                0 => PostgresStrategy::Setup,
                _ => PostgresStrategy::Existing {
                    url: prompt_postgres_url(None).await?,
                },
            }
        };

        Ok(strategy)
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

fn strip_prefixed(features: &[String], prefix: &str) -> Vec<String> {
    features
        .iter()
        .filter_map(|feature| feature.strip_prefix(prefix))
        .map(str::to_string)
        .collect()
}

fn print_summary(
    config: &InstallConfig,
    manifest: &Manifest,
    manifest_path: &std::path::Path,
    release: &Release,
) {
    println!();
    println!(
        "{}",
        Brand::success().bold().apply_to(format!(
            "Malbox {} installed successfully!",
            release.tag_name
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
        if matches!(config.postgres, PostgresStrategy::Setup) {
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
