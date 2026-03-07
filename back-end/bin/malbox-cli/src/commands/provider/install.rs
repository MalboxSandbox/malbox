//! Install a provider.

use crate::commands::Command;
use crate::error::Result;
use clap::Parser;
use malbox_config::Config;

#[derive(Parser)]
#[command(about = "Install a provider (adds to config and rebuilds daemon)")]
pub struct InstallCommand {
    /// Provider name (e.g., "libvirt", "vmware")
    pub name: String,

    /// Skip automatic rebuild (only update config)
    #[arg(long)]
    pub no_rebuild: bool,
}

impl Command for InstallCommand {
    async fn execute(self, config: &Config) -> Result<()> {
        println!("Install provider command - TODO: Implement");
        Ok(())
    }
}
