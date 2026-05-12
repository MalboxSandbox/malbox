use clap::{Parser, Subcommand};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_cli_common::types::OutputFormat;

pub mod completion;
pub mod config;
pub mod daemon;
pub mod image;
pub mod install;
pub mod machine;
pub mod provider;
pub mod upgrade;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "malboxctl - malbox administration tool",
    long_about = "malboxctl - malbox administration tool\n\nInstall, configure, and manage the malbox analysis sandbox infrastructure.",
    after_help = "Use 'malboxctl <command> --help' for more information about a command."
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
    /// Control the malbox daemon
    Daemon(daemon::DaemonCommand),
    /// Manage daemon configuration
    Config(config::ConfigCommand),
    /// Install and set up Malbox
    Install(install::InstallCommand),
    /// Upgrade Malbox to the latest version
    Upgrade(upgrade::UpgradeCommand),
    /// Manage virtualization providers (install, uninstall)
    Provider(provider::ProviderCommand),
    /// Manage analysis machines (list, get, provision, snapshots)
    Machine(machine::MachineCommand),
    /// Manage VM images (register, list, delete)
    Image(image::ImageCommand),
    /// Generate shell completions
    Completion(completion::CompletionCommand),
}

pub use malbox_cli_common::command::Command;

impl Command for Cli {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            Commands::Daemon(cmd) => cmd.execute(ctx).await,
            Commands::Config(cmd) => cmd.execute(ctx).await,
            Commands::Install(cmd) => cmd.execute(ctx).await,
            Commands::Upgrade(cmd) => cmd.execute(ctx).await,
            Commands::Provider(cmd) => cmd.execute(ctx).await,
            Commands::Machine(cmd) => cmd.execute(ctx).await,
            Commands::Image(cmd) => cmd.execute(ctx).await,
            Commands::Completion(cmd) => cmd.execute(ctx).await,
        }
    }
}
