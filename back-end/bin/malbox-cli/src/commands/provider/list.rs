//! List providers compiled into daemon.

use crate::commands::{Command, Context};
use crate::error::Result;
use clap::Parser;

#[derive(Parser)]
#[command(about = "List providers compiled into daemon")]
pub struct ListCommand {}

impl Command for ListCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        println!("Providers compiled into daemon:");

        if ctx.config.providers.enabled.is_empty() {
            println!("  (none)");
            println!();
            println!("Tip: Install a provider with: malbox provider install <name>");
        } else {
            for provider in &ctx.config.providers.enabled {
                // Show default provider with marker
                if ctx.config.providers.get_default() == Some(provider.as_str()) {
                    println!("  ✓ {} (default)", provider);
                } else {
                    println!("  - {}", provider);
                }
            }
            println!();
            println!("Total: {} provider(s)", ctx.config.providers.enabled.len());
        }

        Ok(())
    }
}
