use super::{confirm_reinstall, load_plugin_config};
use crate::utils::install_renderer::InstallRenderer;
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::Brand;
use malbox_plugin_registry::client::RegistryClient;
use malbox_plugin_registry::install::{ExistingInstall, install_resolved_plugin, resolve_plugin};
use malbox_plugin_registry::resolve::{
    InstallStrategy, Platform, RefSelector, RequestedStrategy, SpecifierSource, parse_specifier,
    resolve_selector,
};

#[derive(Parser)]
pub(crate) struct InstallCommand {
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

/// Where the plugin artifact comes from; drives step list, header and summary.
#[derive(Clone, Copy)]
enum InstallMethod {
    Local,
    Source,
    Prebuilt,
}

impl InstallMethod {
    /// Progress renderer titled for this install, with its known steps pre-listed.
    fn renderer(self, name: &str, pin_label: &str) -> InstallRenderer {
        let header = match self {
            Self::Local => format!("\u{27d0} Installing {name} ({pin_label}) from local source"),
            Self::Source => format!("\u{27d0} Installing {name} ({pin_label}) from source"),
            Self::Prebuilt => format!("\u{27d0} Installing {name} ({pin_label})"),
        };
        let renderer = InstallRenderer::new(&header);
        match self {
            // Local steps stream in from install_from_local (build or copy);
            // no fixed pending list, since we do not know which until it runs.
            Self::Local => {}
            Self::Source => renderer.add_pending_steps(&[
                "Cloning repository",
                "Detecting build system",
                "Building release binary",
                "Validating plugin manifest",
                "Installing to plugins directory",
            ]),
            Self::Prebuilt => renderer.add_pending_steps(&[
                "Downloading asset",
                "Extracting archive",
                "Validating plugin manifest",
                "Installing to plugins directory",
            ]),
        }
        renderer
    }

    fn summary(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Source => "from source",
            Self::Prebuilt => "prebuilt",
        }
    }
}

impl InstallCommand {
    /// Validates the CLI ref flags against the specifier and picks the build
    /// strategy, updating `specifier.selector` from `--release/--branch/--rev`.
    fn requested_strategy(
        &self,
        specifier: &mut malbox_plugin_registry::resolve::PluginSpecifier,
    ) -> Result<RequestedStrategy> {
        if matches!(specifier.source, SpecifierSource::Local { .. })
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

        Ok(if is_source_ref || self.source {
            RequestedStrategy::SourceOnly
        } else if self.prebuilt {
            RequestedStrategy::PrebuiltOnly
        } else {
            RequestedStrategy::PrebuiltWithFallback
        })
    }
}

impl Command for InstallCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        let (plugins_config, registry_config) = load_plugin_config().await?;
        let plugins_dir = &plugins_config.directory;
        let platform = Platform::current();

        let mut specifier =
            parse_specifier(&self.plugin).map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let is_local = matches!(specifier.source, SpecifierSource::Local { .. });
        let strategy = self.requested_strategy(&mut specifier)?;

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
            warn_no_prebuilt(&specifier.name, &platform);
            if !ctx.yes && !confirm_build_from_source()? {
                println!("  Installation cancelled.");
                return Ok(());
            }
        }

        let installed = plugins_dir.join(&resolved.name).exists();
        let force = if installed {
            if !(self.force
                || ctx.yes
                || confirm_reinstall(&format!(
                    "{} is already installed. Reinstall {}? ",
                    resolved.name,
                    resolved.pin.label(&resolved.version)
                )))
            {
                println!("  Installation cancelled.");
                return Ok(());
            }
            true
        } else {
            self.force
        };

        let method = if is_local {
            InstallMethod::Local
        } else if matches!(resolved.strategy, InstallStrategy::Source { .. }) {
            InstallMethod::Source
        } else {
            InstallMethod::Prebuilt
        };
        let pin = resolved.pin.clone();

        let renderer = method.renderer(&resolved.name, &pin.label(&resolved.version));

        let existing = if force {
            ExistingInstall::Replace
        } else {
            ExistingInstall::Reject
        };
        let outcome = install_resolved_plugin(resolved, plugins_dir, existing, &renderer)
            .await
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let method = method.summary();
        renderer.finish_line(&format!(
            "{} {} installed ({} plugin, {method})",
            outcome.name,
            pin.label(&outcome.version),
            outcome.plugin_type,
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

fn warn_no_prebuilt(name: &str, platform: &Platform) {
    println!(
        "  {} No prebuilt asset for {} on {}",
        Brand::warning().apply_to("\u{26a0}"),
        name,
        platform.asset_suffix()
    );
}

/// Asks whether to fall back to a source build; `false` means the user declined.
fn confirm_build_from_source() -> Result<bool> {
    dialoguer::Confirm::new()
        .with_prompt("  Build from source?")
        .default(false)
        .interact()
        .map_err(|e| CliError::CommandFailed(e.to_string()))
}
