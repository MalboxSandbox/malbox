use clap::Parser;
use color_eyre::Result;
use malbox_cli_common::api::ApiClient;
use malbox_cli_common::context::Context;
use malbox_tracing::init_tracing;
use tracing::level_filters::LevelFilter;

mod commands;
use commands::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse();

    let log_level = if cli.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::WARN
    };
    init_tracing(log_level);

    // The client CLI only needs the API endpoint; the daemon config is not
    // loaded so the CLI works on machines that only talk to a remote daemon.
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
