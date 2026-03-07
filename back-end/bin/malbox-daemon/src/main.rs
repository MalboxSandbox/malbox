use color_eyre::Result;
use malbox_tracing::init_tracing;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing("debug");
    color_eyre::install()?;

    let config = malbox_config::load_config().await?;
    malbox_daemon::run(config).await?;

    Ok(())
}
