use crate::api::ApiClient;
use crate::error::Result;
use clap::{Parser, Subcommand};
use malbox_config::Config;

pub mod completion;
pub mod config;
pub mod daemon;
pub mod image;
pub mod machine;
pub mod provider;
pub mod task;

pub struct Context {
    pub config: Config,
    pub api: ApiClient,
}

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
    Machine(machine::MachineCommand),
    Image(image::ImageCommand),
    Task(task::TaskCommand),
}

impl Command for Cli {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            Commands::Config(cmd) => cmd.execute(ctx).await,
            Commands::Daemon(cmd) => cmd.execute(ctx).await,
            Commands::Completion(cmd) => cmd.execute(ctx).await,
            Commands::Provider(cmd) => cmd.execute(ctx).await,
            Commands::Machine(cmd) => cmd.execute(ctx).await,
            Commands::Image(cmd) => cmd.execute(ctx).await,
            Commands::Task(cmd) => cmd.execute(ctx).await,
        }
    }
}

pub trait Command {
    fn execute(self, ctx: &Context) -> impl std::future::Future<Output = Result<()>> + Send;
}
