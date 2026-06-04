use crate::commands::Command;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_cli_common::utils::format::{self, Brand};

#[derive(Parser)]
#[command(about = "List providers compiled into daemon")]
pub struct ListCommand {}

impl Command for ListCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let config = malbox_config::load_config().await.map_err(|e| match e {
            malbox_config::ConfigError::NotFound => CliError::CommandFailed(
                "no configuration found - run `malboxctl install` (or `malboxctl config init`) first"
                    .to_string(),
            ),
            e => CliError::CommandFailed(e.to_string()),
        })?;

        let header = Brand::accent().bold();
        println!("{}", header.apply_to("Providers compiled into daemon:"));

        if config.providers.enabled.is_empty() {
            format::empty_with_hint(
                "  (none)",
                "Install a provider with: malboxctl provider install <name>",
            );
        } else {
            let success = Brand::success();
            for provider in &config.providers.enabled {
                if config.providers.get_default() == Some(provider.as_str()) {
                    println!("  {} {} (default)", success.apply_to("\u{2713}"), provider);
                } else {
                    println!("  - {}", provider);
                }
            }
            format::total(config.providers.enabled.len(), "provider");
        }

        Ok(())
    }
}
