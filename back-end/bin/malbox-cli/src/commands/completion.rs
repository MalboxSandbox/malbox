use crate::{
    commands::{Command, Context},
    error::Result,
};
use clap::{CommandFactory, Parser};
use clap_complete::Shell;

#[derive(Parser)]
pub struct CompletionCommand {
    #[arg(value_enum)]
    shell: Shell,
}

impl Command for CompletionCommand {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        let mut cmd = crate::commands::Cli::command();
        clap_complete::generate(self.shell, &mut cmd, "malbox", &mut std::io::stdout());
        Ok(())
    }
}
