use clap::Parser;
use color_eyre::Result;
use malbox_cli::api::ApiClient;
use malbox_cli::commands::{Cli, Command, Context};
use malbox_tracing::init_tracing;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing("debug");

    color_eyre::install()?;

    let config = malbox_config::load_config().await?;
    let cli_config = malbox_config::cli::load_cli_config()
        .map_err(|e| color_eyre::eyre::eyre!("{}", e))?;

    let ctx = Context {
        config: config.clone(),
        api: ApiClient::new(&cli_config.api.url),
    };

    let cli = Cli::parse();

    cli.execute(&ctx)
        .await
        .map_err(|e| color_eyre::eyre::eyre!("{}", e))
}
