use crate::commands::Command;
use clap::{Parser, Subcommand};
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;

mod start;
use start::StartArgs;

#[derive(Parser)]
pub struct DaemonCommand {
    #[command(subcommand)]
    command: DaemonCommands,
}

#[derive(Subcommand)]
pub enum DaemonCommands {
    Start(StartArgs),
}

impl Command for DaemonCommand {
    async fn execute(self, ctx: &Context) -> Result<()> {
        match self.command {
            DaemonCommands::Start(cmd) => cmd.execute(ctx).await,
        }
    }
}
