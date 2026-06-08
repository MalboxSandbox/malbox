use super::load_plugin_config;
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_plugin_registry::client::RegistryClient;
use malbox_plugin_registry::lockfile::Lockfile;

#[derive(Parser)]
pub struct InfoCommand {
    /// Plugin name
    name: String,
}

impl Command for InfoCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let (plugins_config, registry_config) = load_plugin_config().await?;
        let plugins_dir = &plugins_config.directory;

        let lockfile_path = Lockfile::lockfile_path(plugins_dir);
        let lockfile =
            Lockfile::load(&lockfile_path).map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let client = RegistryClient::new(
            &registry_config.repository,
            registry_config.cache_dir.clone(),
        )
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let mut found = false;

        match client.fetch_plugin_metadata(&self.name).await {
            Ok(meta) => {
                found = true;
                println!("  Name:          {}", meta.name);
                println!("  Description:   {}", meta.description);
                println!("  Type:          {}", meta.plugin_type);
                println!("  Repository:    {}", meta.repository);
                if !meta.authors.is_empty() {
                    println!("  Authors:       {}", meta.authors.join(", "));
                }
                if let Some(ref license) = meta.license {
                    println!("  License:       {}", license);
                }
                if !meta.categories.is_empty() {
                    println!("  Categories:    {}", meta.categories.join(", "));
                }
                if !meta.keywords.is_empty() {
                    println!("  Keywords:      {}", meta.keywords.join(", "));
                }
                if let Some(ref ver) = meta.min_malbox_version {
                    println!("  Min malbox:    {}", ver);
                }
                if let Some(ref url) = meta.homepage {
                    println!("  Homepage:      {}", url);
                }
                if !meta.requires.is_empty() {
                    println!("  Dependencies:");
                    for dep in &meta.requires {
                        println!("    - {} {}", dep.name, dep.version);
                    }
                }
            }
            Err(_) => {
                let manifest_path = plugins_dir.join(&self.name).join("plugin.toml");
                if manifest_path.exists() {
                    if let Ok(manifest) = malbox_plugin_manifest::parse_manifest(&manifest_path) {
                        found = true;
                        println!("  Name:          {}", manifest.plugin.name);
                        if let Some(ref desc) = manifest.plugin.description {
                            println!("  Description:   {}", desc);
                        }
                        println!("  Version:       {}", manifest.plugin.version);
                        println!("  Type:          {:?}", manifest.plugin.plugin_type);
                        if !manifest.plugin.authors.is_empty() {
                            println!("  Authors:       {}", manifest.plugin.authors.join(", "));
                        }
                        println!("  (local plugin \u{2014} not in registry)");
                    }
                }
            }
        }

        if let Some(installed) = lockfile.plugins.get(&self.name) {
            found = true;
            println!();
            println!("  Installed:     v{}", installed.version);
            println!("  Install date:  {}", installed.installed_at);

            let source = match installed.source {
                malbox_plugin_registry::lockfile::InstallSource::Registry => "registry",
                malbox_plugin_registry::lockfile::InstallSource::Direct => "direct",
            };
            println!("  Source:        {}", source);
        }

        if !found {
            println!("  Plugin '{}' not found.", self.name);
        }

        Ok(())
    }
}
