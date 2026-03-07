//! Uninstall a provider.

use crate::commands::Command;
use crate::error::Result;
use clap::Parser;
use malbox_config::Config;

#[derive(Parser)]
#[command(about = "Uninstall a provider (removes from config and rebuilds daemon)")]
pub struct UninstallCommand {
    /// Provider name (e.g., "libvirt", "vmware")
    pub name: String,

    /// Skip automatic rebuild (only update config)
    #[arg(long)]
    pub no_rebuild: bool,
}

impl Command for UninstallCommand {
    async fn execute(self, config: &Config) -> Result<()> {
        println!("Uninstall provider command - TODO: Implement");
        Ok(())
    }
}
