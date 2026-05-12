use crate::commands::Command;
use clap::{Parser, Subcommand};
use malbox_cli_common::api::ApiClient;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;
use malbox_cli_common::utils::format::{self, Table, styled_status};

#[derive(Parser)]
#[command(
    about = "View analysis plugins",
    after_help = "Examples:\n  malbox plugin list\n  malbox plugin list --type guest"
)]
pub struct PluginCommand {
    #[command(subcommand)]
    command: PluginCommands,
}

#[derive(Subcommand)]
enum PluginCommands {
    /// List registered plugins
    List(ListArgs),
}

#[derive(Parser)]
struct ListArgs {
    /// Filter by plugin type (e.g., "guest", "host")
    #[arg(long)]
    r#type: Option<String>,
}

impl Command for PluginCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            PluginCommands::List(args) => list(&ctx.api, args).await,
        }
    }
}

async fn list(api: &ApiClient, args: ListArgs) -> Result<()> {
    let plugins = api.list_plugins(args.r#type.as_deref()).await?;

    if plugins.is_empty() {
        format::empty("No plugins found.");
        return Ok(());
    }

    let mut table = Table::new(&["NAME", "VERSION", "TYPE", "STATE", "STATUS", "DESCRIPTION"]);

    for p in &plugins {
        table.add_row(vec![
            p.name.clone(),
            p.version.clone(),
            p.plugin_type.clone(),
            styled_status(&p.state),
            styled_status(&p.status),
            p.description.as_deref().unwrap_or("-").to_string(),
        ]);
    }

    table.print();
    format::total(plugins.len(), "plugin");
    Ok(())
}
