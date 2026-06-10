use super::load_plugin_config;
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_plugin_registry::client::RegistryClient;
use malbox_plugin_registry::install::{install_resolved_plugin, resolve_plugin};
use malbox_plugin_registry::resolve::{
    InstallStrategy, Platform, RequestedStrategy, parse_specifier,
};

#[derive(Parser)]
pub struct InstallCommand {
    /// Plugin name, name@version, or owner/repo
    plugin: String,

    /// Force build from source (skip prebuilt asset search)
    #[arg(long, conflicts_with = "prebuilt")]
    source: bool,

    /// Only use prebuilt assets (error if none available)
    #[arg(long, conflicts_with = "source")]
    prebuilt: bool,

    /// Overwrite existing installation
    #[arg(long)]
    force: bool,

    /// Show what would happen without doing it
    #[arg(long)]
    dry_run: bool,
}

impl Command for InstallCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        let (plugins_config, registry_config) = load_plugin_config().await?;
        let plugins_dir = &plugins_config.directory;
        let platform = Platform::current();

        let specifier =
            parse_specifier(&self.plugin).map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let strategy = if self.source {
            RequestedStrategy::SourceOnly
        } else if self.prebuilt {
            RequestedStrategy::PrebuiltOnly
        } else {
            RequestedStrategy::PrebuiltWithFallback
        };

        let client = RegistryClient::new(
            &registry_config.repository,
            registry_config.cache_dir.clone(),
        )
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        println!("  \u{25b8} Fetching plugin metadata...");

        let resolved = resolve_plugin(&specifier, &client, &platform, strategy)
            .await
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        if matches!(strategy, RequestedStrategy::PrebuiltWithFallback)
            && matches!(resolved.strategy, InstallStrategy::Source { .. })
        {
            println!(
                "  \u{26a0} No prebuilt asset for {} on {}",
                specifier.name,
                platform.asset_suffix()
            );
            if !ctx.yes {
                let confirmed = dialoguer::Confirm::new()
                    .with_prompt("  Build from source?")
                    .default(false)
                    .interact()
                    .map_err(|e| CliError::CommandFailed(e.to_string()))?;
                if !confirmed {
                    println!("  Installation cancelled.");
                    return Ok(());
                }
            }
        }

        if self.dry_run {
            let method = match &resolved.strategy {
                InstallStrategy::Prebuilt { asset, .. } => {
                    let size_mb = asset.size as f64 / 1_048_576.0;
                    format!("prebuilt ({}, {:.1} MB)", asset.name, size_mb)
                }
                InstallStrategy::Source { clone_url, .. } => {
                    format!("source ({})", clone_url)
                }
            };
            println!(
                "  \u{25b8} Would install {} v{} via {}",
                resolved.name, resolved.version, method
            );
            println!(
                "  \u{25b8} Target: {}",
                plugins_dir.join(&resolved.name).display()
            );
            println!("  (dry run \u{2014} no changes made)");
            return Ok(());
        }

        let is_source = matches!(resolved.strategy, InstallStrategy::Source { .. });

        if is_source {
            println!("  \u{25b8} Building from source...");
        }

        let outcome = install_resolved_plugin(
            resolved,
            plugins_dir,
            self.force,
            &mut |downloaded, total| {
                if let Some(total) = total {
                    let mb_done = downloaded as f64 / 1_048_576.0;
                    let mb_total = total as f64 / 1_048_576.0;
                    eprint!(
                        "\r  \u{25b8} Downloading... {:.1}/{:.1} MB",
                        mb_done, mb_total
                    );
                }
            },
        )
        .await
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        if !is_source {
            eprintln!();
        }

        let method = if is_source { "from source" } else { "prebuilt" };
        println!(
            "  \u{2713} Installed {} v{} ({} plugin, {})",
            outcome.name, outcome.version, outcome.plugin_type, method
        );

        if !outcome.missing_deps.is_empty() {
            println!();
            for dep in &outcome.missing_deps {
                println!(
                    "  \u{26a0} Missing optional dependency: {} {}",
                    dep.name, dep.version
                );
                println!(
                    "    Install with: malbox daemon plugin install {}",
                    dep.name
                );
            }
        }

        Ok(())
    }
}
