use crate::api::ApiClient;
use crate::error::Result;
use crate::types::OutputFormat;
use clap::{Parser, Subcommand};
use malbox_config::Config;

pub mod completion;
pub mod config;
pub mod daemon;
pub mod image;
pub mod machine;
pub mod plugin;
pub mod provider;
pub mod task;

pub struct Context {
    pub config: Config,
    pub api: ApiClient,
    pub format: OutputFormat,
    pub verbose: bool,
}

#[derive(Parser)]
#[command(author, version, about)]
pub struct Cli {
    /// Output format
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    /// Override API base URL from config
    #[arg(long, global = true)]
    pub api_url: Option<String>,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

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
    Plugin(plugin::PluginCommand),
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
            Commands::Plugin(cmd) => cmd.execute(ctx).await,
        }
    }
}

pub trait Command {
    fn execute(self, ctx: &Context) -> impl std::future::Future<Output = Result<()>> + Send;
}
