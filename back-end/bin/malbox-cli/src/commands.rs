use crate::api::ApiClient;
use crate::error::Result;
use crate::types::OutputFormat;
use clap::{Parser, Subcommand};
use malbox_config::Config;

pub mod completion;
pub mod config;
pub mod daemon;
pub mod image;
pub mod install;
pub mod machine;
pub mod plugin;
pub mod provider;
pub mod task;
pub mod upgrade;

pub struct Context {
    pub config: Config,
    pub api: ApiClient,
    pub format: OutputFormat,
    pub verbose: bool,
    pub yes: bool,
}

#[derive(Parser)]
#[command(
    author,
    version,
    about = "malbox - malware analysis sandbox",
    long_about = "malbox - malware analysis sandbox\n\nSubmit samples, manage analysis machines, and retrieve results.",
    after_help = "Use 'malbox <command> --help' for more information about a command."
)]
pub struct Cli {
    /// Output format (text or json)
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    /// Override API base URL from config
    #[arg(long, global = true)]
    pub api_url: Option<String>,

    /// Enable verbose output (shows timestamps, debug info)
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Skip confirmation prompts (for scripting)
    #[arg(short, long, global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage analysis machines (list, get, provision, snapshots)
    Machine(machine::MachineCommand),
    /// Submit samples and manage analysis tasks
    Task(task::TaskCommand),
    /// Manage VM images (register, list, delete)
    Image(image::ImageCommand),
    /// List installed analysis plugins
    Plugin(plugin::PluginCommand),
    /// Manage virtualization providers (install, uninstall)
    Provider(provider::ProviderCommand),
    /// Manage daemon configuration
    Config(config::ConfigCommand),
    /// Control the malbox daemon
    Daemon(daemon::DaemonCommand),
    /// Install and set up Malbox
    Install(install::InstallCommand),
    /// Upgrade Malbox to the latest version
    Upgrade(upgrade::UpgradeCommand),
    /// Generate shell completions
    Completion(completion::CompletionCommand),
}

impl Command for Cli {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            Commands::Config(cmd) => cmd.execute(ctx).await,
            Commands::Daemon(cmd) => cmd.execute(ctx).await,
            Commands::Install(cmd) => cmd.execute(ctx).await,
            Commands::Upgrade(cmd) => cmd.execute(ctx).await,
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
