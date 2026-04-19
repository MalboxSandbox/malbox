use crate::commands::{Command, Context};
use crate::error::Result;
use clap::{Parser, Subcommand};

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
