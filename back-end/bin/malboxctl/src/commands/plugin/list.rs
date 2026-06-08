use super::load_plugin_config;
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_plugin_manifest::parse_manifest;
use malbox_plugin_registry::lockfile::Lockfile;

#[derive(Parser)]
pub struct ListCommand;

impl Command for ListCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let (plugins_config, _) = load_plugin_config().await?;
        let plugins_dir = &plugins_config.directory;
        let lockfile_path = Lockfile::lockfile_path(plugins_dir);
        let lockfile = Lockfile::load(&lockfile_path)
            .map_err(|e| malbox_cli_common::error::CliError::CommandFailed(e.to_string()))?;

        let untracked = lockfile
            .untracked_plugins(plugins_dir)
            .map_err(|e| malbox_cli_common::error::CliError::CommandFailed(e.to_string()))?;

        if lockfile.plugins.is_empty() && untracked.is_empty() {
            println!("  No plugins installed.");
            return Ok(());
        }

        println!(
            "  {:<20} {:<10} {:<8} {:<12} SOURCE",
            "NAME", "VERSION", "TYPE", "STATUS"
        );

        for (name, plugin) in &lockfile.plugins {
            let manifest_path = plugins_dir.join(name).join("plugin.toml");
            let plugin_type = if manifest_path.exists() {
                parse_manifest(&manifest_path)
                    .map(|m| format!("{:?}", m.plugin.plugin_type).to_lowercase())
                    .unwrap_or_else(|_| "?".into())
            } else {
                "?".into()
            };

            let source = match plugin.source {
                malbox_plugin_registry::lockfile::InstallSource::Registry => "registry",
                malbox_plugin_registry::lockfile::InstallSource::Direct => "direct",
            };

            println!(
                "  {:<20} {:<10} {:<8} {:<12} {}",
                name, plugin.version, plugin_type, "tracked", source
            );
        }

        for name in &untracked {
            let plugin_type = parse_manifest(&plugins_dir.join(name).join("plugin.toml"))
                .map(|m| format!("{:?}", m.plugin.plugin_type).to_lowercase())
                .unwrap_or_else(|_| "?".into());

            let version = parse_manifest(&plugins_dir.join(name).join("plugin.toml"))
                .map(|m| m.plugin.version)
                .unwrap_or_else(|_| "?".into());

            println!(
                "  {:<20} {:<10} {:<8} {:<12} \u{2014}",
                name, version, plugin_type, "untracked"
            );
        }

        let total = lockfile.plugins.len() + untracked.len();
        println!("\n  {} plugin(s) installed", total);

        Ok(())
    }
}
