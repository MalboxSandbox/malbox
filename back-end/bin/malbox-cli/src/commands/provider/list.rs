//! List providers compiled into daemon.

use crate::commands::Command;
use crate::error::Result;
use clap::Parser;
use malbox_config::Config;

#[derive(Parser)]
#[command(about = "List providers compiled into daemon")]
pub struct ListCommand {}

impl Command for ListCommand {
    async fn execute(self, config: &Config) -> Result<()> {
        println!("Providers compiled into daemon:");

        if config.providers.enabled.is_empty() {
            println!("  (none)");
            println!();
            println!("Tip: Install a provider with: malbox provider install <name>");
        } else {
            for provider in &config.providers.enabled {
                // Show default provider with marker
                if config.providers.get_default() == Some(provider.as_str()) {
                    println!("  ✓ {} (default)", provider);
                } else {
                    println!("  - {}", provider);
                }
            }
            println!();
            println!("Total: {} provider(s)", config.providers.enabled.len());
        }

        Ok(())
    }
}
