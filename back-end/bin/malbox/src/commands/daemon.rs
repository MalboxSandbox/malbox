use crate::commands::Command;
use clap::{Parser, Subcommand};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;

pub mod config;
pub mod install;
pub mod plugin;
pub mod provider;
mod start;
pub mod upgrade;

#[derive(Parser)]
#[command(
    about = "Manage the malbox daemon",
    long_about = "Manage the malbox daemon installation, configuration, providers, and plugins.\n\n\
                  These commands run on the analysis host and manage the local daemon installation.\n\
                  For analysis operations, use the top-level commands (task, machine, image)."
)]
pub struct DaemonCommand {
    #[command(subcommand)]
    pub command: DaemonCommands,
}

#[derive(Subcommand)]
pub enum DaemonCommands {
    /// Start the malbox daemon
    Start(start::StartArgs),
    /// Install and set up malbox
    Install(install::InstallCommand),
    /// Upgrade malbox to the latest version
    Upgrade(upgrade::UpgradeCommand),
    /// Manage daemon configuration
    Config(config::ConfigCommand),
    /// Manage virtualization providers
    Provider(provider::ProviderCommand),
    /// Manage installed plugins on this host
    Plugin(plugin::PluginCommand),
}

impl Command for DaemonCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            DaemonCommands::Start(cmd) => cmd.execute(ctx).await,
            DaemonCommands::Install(cmd) => cmd.execute(ctx).await,
            DaemonCommands::Upgrade(cmd) => cmd.execute(ctx).await,
            DaemonCommands::Config(cmd) => cmd.execute(ctx).await,
            DaemonCommands::Provider(cmd) => cmd.execute(ctx).await,
            DaemonCommands::Plugin(cmd) => cmd.execute(ctx).await,
        }
    }
}
