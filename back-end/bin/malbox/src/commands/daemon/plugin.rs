mod info;
mod install;
mod list;
mod remove;
mod search;
mod update;

use clap::{Parser, Subcommand};
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_config::{PluginsConfig, RegistryConfig};

#[derive(Parser)]
#[command(about = "Manage installed plugins on this host")]
pub struct PluginCommand {
    #[command(subcommand)]
    command: PluginCommands,
}

#[derive(Subcommand)]
enum PluginCommands {
    /// Install a plugin from the registry or a GitHub repository
    Install(install::InstallCommand),
    /// Remove an installed plugin
    Remove(remove::RemoveCommand),
    /// List installed plugins
    List(list::ListCommand),
    /// Search the plugin registry
    Search(search::SearchCommand),
    /// Show detailed information about a plugin
    Info(info::InfoCommand),
    /// Update installed plugins to their latest versions
    Update(update::UpdateCommand),
}

impl Command for PluginCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            PluginCommands::Install(cmd) => cmd.execute(ctx).await,
            PluginCommands::Remove(cmd) => cmd.execute(ctx).await,
            PluginCommands::List(cmd) => cmd.execute(ctx).await,
            PluginCommands::Search(cmd) => cmd.execute(ctx).await,
            PluginCommands::Info(cmd) => cmd.execute(ctx).await,
            PluginCommands::Update(cmd) => cmd.execute(ctx).await,
        }
    }
}

/// Interactive yes/no for reinstalling something already present. Unlike the
/// shared `format::confirm`, a non-interactive terminal defaults to No so a
/// scripted run never silently rebuilds. Pass `skip = true` (from `--force`/`-y`)
/// to proceed without asking.
pub(crate) fn confirm_reinstall(prompt: &str, skip: bool) -> bool {
    use std::io::IsTerminal;
    if skip {
        return true;
    }
    if !std::io::stderr().is_terminal() {
        return false;
    }
    dialoguer::Confirm::with_theme(&malbox_cli_common::utils::format::malbox_theme())
        .with_prompt(prompt)
        .default(false)
        .interact()
        .unwrap_or(false)
}

pub(crate) async fn load_plugin_config() -> Result<(PluginsConfig, RegistryConfig)> {
    match malbox_config::load_config().await {
        Ok(config) => {
            let registry = config.plugins.registry.clone().unwrap_or_default();
            Ok((config.plugins.clone(), registry))
        }
        Err(malbox_config::ConfigError::NotFound) => {
            Ok((PluginsConfig::default(), RegistryConfig::default()))
        }
        Err(e) => Err(e.into()),
    }
}
