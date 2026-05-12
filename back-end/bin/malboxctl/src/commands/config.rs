use crate::commands::Command;
use clap::{Parser, Subcommand};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;

mod init;
pub use init::InitArgs;

#[derive(Parser)]
pub struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigCommands,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Initialize a new configuration file with defaults
    Init(InitArgs),
}

impl Command for ConfigCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            ConfigCommands::Init(args) => args.execute(ctx).await,
        }
    }
}
