use clap::Parser;
use color_eyre::Result;
use malbox_cli_common::api::ApiClient;
use malbox_cli_common::context::Context;
use malbox_tracing::init_tracing;
use tracing::level_filters::LevelFilter;

mod commands;
mod utils;
use commands::daemon::DaemonCommands;
use commands::{Cli, Command, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    let log_level = match &cli.command {
        Commands::Daemon(d) => match &d.command {
            DaemonCommands::Start(args) => args.log_level,
        },
        _ => {
            if cli.verbose {
                LevelFilter::DEBUG
            } else {
                LevelFilter::WARN
            }
        }
    };
    init_tracing(log_level);

    // The daemon config is deliberately NOT loaded here: commands that need
    // it load it lazily, so first-run commands (`install`, `config init`,
    // completions) work before any configuration exists. The CLI config
    // already falls back to defaults when absent.
    let cli_config =
        malbox_config::cli::load_cli_config().map_err(|e| color_eyre::eyre::eyre!("{}", e))?;
    let api_url = cli.api_url.as_deref().unwrap_or(&cli_config.api.url);

    let ctx = Context {
        api: ApiClient::new(api_url),
        format: cli.format.clone(),
        verbose: cli.verbose,
        yes: cli.yes,
    };

    cli.execute(&ctx)
        .await
        .map_err(|e| color_eyre::eyre::eyre!("{}", e))
}
