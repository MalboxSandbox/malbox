use super::load_plugin_config;
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_plugin_registry::lockfile::Lockfile;
use malbox_plugin_registry::remove::remove_plugin;

#[derive(Parser)]
pub struct RemoveCommand {
    /// Plugin name to remove
    name: String,
}

impl Command for RemoveCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let (plugins_config, _) = load_plugin_config().await?;
        let plugins_dir = &plugins_config.directory;

        let lockfile_path = Lockfile::lockfile_path(plugins_dir);
        let lockfile =
            Lockfile::load(&lockfile_path).map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let version = lockfile
            .plugins
            .get(&self.name)
            .map(|p| format!(" {}", p.pin.label(&p.version)))
            .unwrap_or_default();

        println!("  \u{25b8} Removing {}{}...", self.name, version);

        remove_plugin(&self.name, plugins_dir)
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        println!("  \u{2713} Removed {}", self.name);

        Ok(())
    }
}
