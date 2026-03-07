//! Provider management commands.
//!
//! Manages which provider crates are compiled into the malbox daemon.
//! Providers are installed/uninstalled via Cargo features and configuration.

use crate::commands::Command;
use crate::error::Result;
use clap::{Parser, Subcommand};
use malbox_config::Config;

pub mod install;
pub mod list;
pub mod rebuild;
pub mod uninstall;

pub use install::InstallCommand;
pub use list::ListCommand;
pub use rebuild::RebuildCommand;
pub use uninstall::UninstallCommand;

#[derive(Parser)]
#[command(about = "Manage virtualization providers")]
pub struct ProviderCommand {
    #[command(subcommand)]
    command: ProviderCommands,
}

#[derive(Subcommand)]
enum ProviderCommands {
    /// List providers compiled into daemon
    List(ListCommand),
    /// Install a provider
    Install(InstallCommand),
    /// Uninstall a provider
    Uninstall(UninstallCommand),
    /// Rebuild daemon with current provider configuration
    Rebuild(RebuildCommand),
}

impl Command for ProviderCommand {
    async fn execute(self, config: &Config) -> Result<()> {
        match self.command {
            ProviderCommands::List(cmd) => cmd.execute(config).await,
            ProviderCommands::Install(cmd) => cmd.execute(config).await,
            ProviderCommands::Uninstall(cmd) => cmd.execute(config).await,
            ProviderCommands::Rebuild(cmd) => cmd.execute(config).await,
        }
    }
}
