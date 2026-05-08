use crate::commands::{Command, Context};
use crate::error::Result;
use crate::utils::format;
use clap::Parser;

#[derive(Parser)]
#[command(
    about = "Uninstall a provider (removes from config and rebuilds daemon)",
    after_help = "Examples:\n  malbox provider uninstall libvirt\n  malbox provider uninstall vmware --no-rebuild"
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
