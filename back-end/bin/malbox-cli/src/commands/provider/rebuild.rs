//! Rebuild daemon with current provider configuration.

use crate::commands::{Command, Context};
use crate::error::Result;
use clap::Parser;

#[derive(Parser)]
#[command(about = "Rebuild daemon with current provider configuration")]
pub struct RebuildCommand {
    /// Build in release mode
    #[arg(short, long)]
    pub release: bool,

    /// Show verbose cargo output
    #[arg(short, long)]
    pub verbose: bool,
}

impl Command for RebuildCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        println!("Rebuild command - TODO: Implement");
        Ok(())
    }
}
