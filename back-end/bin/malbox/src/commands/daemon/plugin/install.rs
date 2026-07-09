use super::{confirm_reinstall, load_plugin_config};
use crate::utils::install_renderer::InstallRenderer;
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::Brand;
use malbox_plugin_registry::client::RegistryClient;
use malbox_plugin_registry::install::{install_resolved_plugin, resolve_plugin};
use malbox_plugin_registry::resolve::{
    InstallStrategy, Platform, RefSelector, RequestedStrategy, SpecifierSource, parse_specifier,
    resolve_selector,
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

    /// Install a specific published release (e.g. 1.2.3)
    #[arg(long, value_name = "VERSION", conflicts_with_all = ["branch", "rev"])]
    release: Option<String>,

    /// Build from a branch's latest commit (source build)
    #[arg(long, value_name = "NAME", conflicts_with_all = ["release", "rev", "prebuilt"])]
    branch: Option<String>,

    /// Build from a specific commit; expects a full 40-char SHA (source build)
    #[arg(long, value_name = "SHA", conflicts_with_all = ["release", "branch", "prebuilt"])]
    rev: Option<String>,

    /// Overwrite existing installation
    #[arg(long)]
    force: bool,
}

impl Command for InstallCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        let (plugins_config, registry_config) = load_plugin_config().await?;
        let plugins_dir = &plugins_config.directory;
        let platform = Platform::current();

        let mut specifier =
            parse_specifier(&self.plugin).map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let is_local = matches!(specifier.source, SpecifierSource::Local { .. });
        if is_local
            && (self.release.is_some()
                || self.branch.is_some()
                || self.rev.is_some()
                || self.source
                || self.prebuilt)
        {
            return Err(CliError::CommandFailed(
                "--release/--branch/--rev/--source/--prebuilt do not apply to a local path".into(),
            ));
        }

        specifier.selector = resolve_selector(
            specifier.selector.clone(),
            self.release.clone(),
            self.branch.clone(),
            self.rev.clone(),
        )
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let is_source_ref = matches!(
            specifier.selector,
            RefSelector::Branch(_) | RefSelector::Commit(_)
        );

        let strategy = if is_source_ref || self.source {
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

        if is_local {
            println!("  \u{25b8} Reading local plugin...");
        } else {
            println!("  \u{25b8} Fetching plugin metadata...");
        }

        let resolved = resolve_plugin(&specifier, &client, &platform, strategy)
            .await
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        if matches!(strategy, RequestedStrategy::PrebuiltWithFallback)
            && matches!(resolved.strategy, InstallStrategy::Source { .. })
        {
            let warn = Brand::warning();
            println!(
                "  {} No prebuilt asset for {} on {}",
                warn.apply_to("\u{26a0}"),
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

        let installed = plugins_dir.join(&resolved.name).exists();
        let force = if installed {
            if !confirm_reinstall(
                &format!(
                    "{} is already installed. Reinstall {}? ",
                    resolved.name,
                    resolved.pin.label(&resolved.version)
                ),
                self.force || ctx.yes,
            ) {
                println!("  Installation cancelled.");
                return Ok(());
            }
            true
        } else {
            self.force
        };

        let is_source = matches!(resolved.strategy, InstallStrategy::Source { .. });
        let pin = resolved.pin.clone();

        let header = if is_local {
            format!(
                "\u{27d0} Installing {} ({}) from local source",
                resolved.name,
                pin.label(&resolved.version)
            )
        } else if is_source {
            format!(
                "\u{27d0} Installing {} ({}) from source",
                resolved.name,
                pin.label(&resolved.version)
            )
        } else {
            format!(
                "\u{27d0} Installing {} ({})",
                resolved.name,
                pin.label(&resolved.version)
            )
        };

        let renderer = InstallRenderer::new(&header);

        if is_local {
            // Local steps stream in from install_from_local (build or copy);
            // no fixed pending list, since we do not know which until it runs.
        } else if is_source {
            renderer.add_pending_steps(&[
                "Cloning repository",
                "Detecting build system",
                "Building release binary",
                "Validating plugin manifest",
                "Installing to plugins directory",
            ]);
        } else {
            renderer.add_pending_steps(&[
                "Downloading asset",
                "Extracting archive",
                "Validating plugin manifest",
                "Installing to plugins directory",
            ]);
        }

        let outcome = install_resolved_plugin(resolved, plugins_dir, force, &renderer)
            .await
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let method = if is_local {
            "local"
        } else if is_source {
            "from source"
        } else {
            "prebuilt"
        };
        renderer.finish_line(&format!(
            "{} {} installed ({} plugin, {})",
            outcome.name,
            pin.label(&outcome.version),
            outcome.plugin_type,
            method
        ));

        if !outcome.missing_deps.is_empty() {
            let warn = Brand::warning();
            let dim = Brand::dim();
            println!();
            for dep in &outcome.missing_deps {
                println!(
                    "  {} {} {} {}",
                    warn.apply_to("\u{26a0}"),
                    warn.apply_to("Missing optional dependency:"),
                    dep.name,
                    dim.apply_to(&dep.version),
                );
                println!(
                    "    {}",
                    dim.apply_to(format!(
                        "Install with: malbox daemon plugin install {}",
                        dep.name
                    ))
                );
            }
        }

        Ok(())
    }
}
