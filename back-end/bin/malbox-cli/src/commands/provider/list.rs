//! List providers compiled into daemon.

use crate::commands::{Command, Context};
use crate::error::Result;
use crate::utils::format;
use clap::Parser;
use console::Style;

#[derive(Parser)]
#[command(about = "List providers compiled into daemon")]
pub struct ListCommand {}

impl Command for ListCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        let bold = Style::new().bold();
        println!("{}", bold.apply_to("Providers compiled into daemon:"));

        if ctx.config.providers.enabled.is_empty() {
            format::empty("  (none)");
            println!();
            let dim = Style::new().dim();
            println!(
                "{}",
                dim.apply_to("Tip: Install a provider with: malbox provider install <name>")
            );
        } else {
            let green = Style::new().green();
            for provider in &ctx.config.providers.enabled {
                if ctx.config.providers.get_default() == Some(provider.as_str()) {
                    println!("  {} {} (default)", green.apply_to("\u{2713}"), provider);
                } else {
                    println!("  - {}", provider);
                }
            }
            format::total(ctx.config.providers.enabled.len(), "provider");
        }

        Ok(())
    }
}
