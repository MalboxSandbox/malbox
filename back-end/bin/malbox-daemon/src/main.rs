use clap::Parser;
use color_eyre::Result;
use malbox_tracing::{init_tracing, parse_log_level};
use tracing::level_filters::LevelFilter;

/// malbox daemon
#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Args {
    /// Log level (error | warn | info | debug | trace). Overridden by RUST_LOG.
    #[arg(long, default_value = "info", value_parser = parse_log_level)]
    log_level: LevelFilter,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    init_tracing(args.log_level);
    color_eyre::install()?;

    let config = malbox_config::load_config().await?;
    malbox_daemon::run(config).await?;

    Ok(())
}
