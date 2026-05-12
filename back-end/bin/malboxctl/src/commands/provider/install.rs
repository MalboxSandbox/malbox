//! Install a provider.

use crate::commands::Command;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;

#[derive(Parser)]
#[command(
    about = "Install a provider (adds to config and rebuilds daemon)",
    after_help = "Examples:\n  malboxctl provider install libvirt\n  malboxctl provider install vmware --no-rebuild"
)]
pub struct InstallCommand {
    /// Provider name (e.g., "libvirt", "vmware")
    pub name: String,

    /// Skip automatic rebuild (only update config)
    #[arg(long)]
    pub no_rebuild: bool,
}

impl Command for InstallCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        println!("Install provider command - TODO: Implement");
        Ok(())
    }
}
