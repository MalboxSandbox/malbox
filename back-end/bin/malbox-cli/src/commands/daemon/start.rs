use crate::{
    commands::{Command, Context},
    error::Result,
};
use clap::Parser;

#[derive(Parser)]
pub struct StartArgs {
    #[arg(short, long)]
    pub config_path: Option<String>,
}

// NOTE:
// We should use the Spinner (rattles diagswipe) to show when the service is starting/started.
// We might need to split the daemon `run` function into different parts to get more precise loading states.
// It's also worth to consider making a Daemon struct in malbox-daemon, and implement the different methods there, instead of a single `run` function.
impl Command for StartArgs {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        todo!()
    }
}
