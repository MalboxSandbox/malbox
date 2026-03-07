use crate::commands::Command;
use crate::error::Result;
use clap::{Parser, Subcommand};
use malbox_config::Config;

mod init;
mod playbook;
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
    async fn execute(self, config: &Config) -> Result<()> {
        match self.command {
            ConfigCommands::Init(args) => args.execute(config).await,
        }
    }
}
