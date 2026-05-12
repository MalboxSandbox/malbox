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

    let config = malbox_config::load_config().await?;
    let cli_config =
        malbox_config::cli::load_cli_config().map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

    let cli = Cli::parse();

    let log_level = if cli.verbose {
        LevelFilter::DEBUG
    } else {
        LevelFilter::WARN
    };
    init_tracing(log_level);

    let api_url = cli.api_url.as_deref().unwrap_or(&cli_config.api.url);

    let ctx = Context {
        config: config.clone(),
        api: ApiClient::new(api_url),
        format: cli.format.clone(),
        verbose: cli.verbose,
        yes: cli.yes,
    };

    cli.execute(&ctx)
        .await
        .map_err(|e| color_eyre::eyre::eyre!("{}", e))
}
