use crate::commands::Command;
use crate::utils::provider_config::ProvidersEdit;
use clap::Parser;
use console::style;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::{self, Brand};
use malbox_installer::manifest::Manifest;

#[derive(Parser)]
#[command(
    about = "Uninstall a provider (removes from config and rebuilds daemon)",
    after_help = "Examples:\n  malbox daemon provider uninstall libvirt\n  malbox daemon provider uninstall xen --no-rebuild"
)]
pub struct UninstallCommand {
    /// Provider name (e.g., "libvirt", "xen")
    pub name: String,

    /// Skip automatic rebuild (only update config)
    #[arg(long)]
    pub no_rebuild: bool,
}

impl Command for UninstallCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        let prompt = format!("Uninstall provider '{}'?", self.name);
        if !format::confirm(&prompt, ctx.yes) {
            format::empty("Cancelled.");
            return Ok(());
        }

        let mut edit = ProvidersEdit::load()?;
        if !edit.remove(&self.name) {
            return Err(CliError::CommandFailed(format!(
                "provider '{}' is not enabled",
                self.name
            )));
        }
        edit.save()?;
        println!(
            "  {} disabled '{}' in {}",
            Brand::success().apply_to("\u{2713}"),
            self.name,
            edit.path().display()
        );

        let remaining = edit.enabled();
        if remaining.is_empty() {
            println!(
                "  {} no providers remain enabled - the daemon will refuse to start until one is installed",
                Brand::warning().apply_to("!")
            );
        }

        if self.no_rebuild {
            println!(
                "  Skipping rebuild - run {} to drop it from the daemon",
                style("malbox daemon provider rebuild").bold()
            );
            return Ok(());
        }

        let manifest = Manifest::load(&Manifest::default_path())
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;
        let features = super::desired_features(&manifest, &remaining)?;
        if super::binary_matches(&manifest, &features) {
            println!("  Daemon already carries this provider set - nothing to rebuild.");
            return Ok(());
        }

        super::rebuild_daemon(&features).await
    }
}
