use crate::commands::Command;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_cli_common::utils::format;

#[derive(Parser)]
#[command(
    about = "Uninstall a provider (removes from config and rebuilds daemon)",
    after_help = "Examples:\n  malboxctl provider uninstall libvirt\n  malboxctl provider uninstall vmware --no-rebuild"
)]
pub struct UninstallCommand {
    /// Provider name (e.g., "libvirt", "vmware")
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

        println!("Uninstall provider command - TODO: Implement");
        Ok(())
    }
}
