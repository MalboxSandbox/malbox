use clap::{Parser, Subcommand};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_cli_common::types::OutputFormat;

pub mod completion;
pub mod machine;
pub mod plugin;
pub mod task;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "malbox - malware analysis sandbox",
    long_about = "malbox - malware analysis sandbox\n\nSubmit samples, manage analysis tasks, and retrieve results.",
    after_help = "Use 'malbox <command> --help' for more information about a command."
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    #[arg(long, global = true)]
    pub api_url: Option<String>,

    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[arg(short, long, global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage analysis tasks
    Task(task::TaskCommand),
    /// View analysis machines
    Machine(machine::MachineCommand),
    /// View analysis plugins
    Plugin(plugin::PluginCommand),
    /// Generate shell completions
    Completion(completion::CompletionCommand),
}

pub use malbox_cli_common::command::Command;

impl Command for Cli {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            Commands::Task(cmd) => cmd.execute(ctx).await,
            Commands::Machine(cmd) => cmd.execute(ctx).await,
            Commands::Plugin(cmd) => cmd.execute(ctx).await,
            Commands::Completion(cmd) => cmd.execute(ctx).await,
        }
    }
}
