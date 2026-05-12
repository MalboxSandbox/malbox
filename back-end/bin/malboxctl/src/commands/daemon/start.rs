use crate::commands::Command;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::Result;

#[derive(Parser)]
pub struct StartArgs {
    #[arg(short, long)]
    pub config_path: Option<String>,
}

impl Command for StartArgs {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        todo!()
    }
}
