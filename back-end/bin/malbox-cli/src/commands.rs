use crate::error::Result;
use clap::{Parser, Subcommand};
use malbox_config::Config;

pub mod completion;
pub mod config;
pub mod daemon;
pub mod provider;

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Config(config::ConfigCommand),
    Daemon(daemon::DaemonCommand),
    Completion(completion::CompletionCommand),
    Provider(provider::ProviderCommand),
}

impl Command for Cli {
    async fn execute(self, config: &Config) -> Result<()> {
        match self.command {
            Commands::Config(cmd) => cmd.execute(config).await,
            Commands::Daemon(cmd) => cmd.execute(config).await,
            Commands::Completion(cmd) => cmd.execute(config).await,
            Commands::Provider(cmd) => cmd.execute(config).await,
        }
    }
}

pub trait Command {
    async fn execute(self, config: &Config) -> Result<()>;
}
