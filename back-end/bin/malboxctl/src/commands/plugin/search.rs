use super::load_plugin_config;
use clap::Parser;
use malbox_cli_common::command::Command;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_plugin_registry::client::RegistryClient;

#[derive(Parser)]
pub struct SearchCommand {
    /// Search query (matches name, description, and categories)
    query: String,
}

impl Command for SearchCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let (_, registry_config) = load_plugin_config().await?;

        let client = RegistryClient::new(
            &registry_config.repository,
            registry_config.cache_dir.clone(),
        )
        .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let index = client
            .fetch_index()
            .await
            .map_err(|e| CliError::CommandFailed(e.to_string()))?;

        let results = index.search(&self.query);

        if results.is_empty() {
            println!("  No plugins found matching '{}'", self.query);
            return Ok(());
        }

        println!("  {:<20} {:<52} {}", "NAME", "DESCRIPTION", "TYPE");

        for entry in &results {
            let desc = if entry.description.len() > 50 {
                format!("{}...", &entry.description[..47])
            } else {
                entry.description.clone()
            };
            println!("  {:<20} {:<52} {}", entry.name, desc, entry.plugin_type);
        }

        println!("\n  {} plugin(s) found", results.len());

        Ok(())
    }
}
