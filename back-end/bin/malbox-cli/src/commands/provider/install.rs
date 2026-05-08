//! Install a provider.

use crate::commands::{Command, Context};
use crate::error::Result;
use clap::Parser;

#[derive(Parser)]
#[command(
    about = "Install a provider (adds to config and rebuilds daemon)",
    after_help = "Examples:\n  malbox provider install libvirt\n  malbox provider install vmware --no-rebuild"
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
