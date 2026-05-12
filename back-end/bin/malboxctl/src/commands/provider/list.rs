use crate::commands::Command;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_cli_common::utils::format::{self, Brand};

#[derive(Parser)]
#[command(about = "List providers compiled into daemon")]
pub struct ListCommand {}

impl Command for ListCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        let header = Brand::accent().bold();
        println!("{}", header.apply_to("Providers compiled into daemon:"));

        if ctx.config.providers.enabled.is_empty() {
            format::empty_with_hint(
                "  (none)",
                "Install a provider with: malboxctl provider install <name>",
            );
        } else {
            let success = Brand::success();
            for provider in &ctx.config.providers.enabled {
                if ctx.config.providers.get_default() == Some(provider.as_str()) {
                    println!("  {} {} (default)", success.apply_to("\u{2713}"), provider);
                } else {
                    println!("  - {}", provider);
                }
            }
            format::total(ctx.config.providers.enabled.len(), "provider");
        }

        Ok(())
    }
}
