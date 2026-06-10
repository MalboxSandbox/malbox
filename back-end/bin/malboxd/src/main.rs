use clap::Parser;
use color_eyre::Result;
use malbox_tracing::{init_tracing, parse_log_level};
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;

/// Malbox analysis daemon.
#[derive(Parser)]
#[command(name = "malboxd", about = "Malbox analysis daemon")]
struct Args {
    /// Log level (error | warn | info | debug | trace). Overridden by RUST_LOG.
    #[arg(long, default_value = "info", value_parser = parse_log_level)]
    log_level: LevelFilter,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    init_tracing(args.log_level);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let config = malbox_config::load_config().await.map_err(|e| match e {
            malbox_config::ConfigError::NotFound => color_eyre::eyre::eyre!(
                "no configuration found - run `malbox daemon install` (or `malbox daemon config init`) first"
            ),
            e => color_eyre::eyre::eyre!("{}", e),
        })?;

        let shutdown_token = CancellationToken::new();

        malbox_daemon::run(config, shutdown_token)
            .await
            .map_err(|e| color_eyre::eyre::eyre!("{}", e))
    })
}
