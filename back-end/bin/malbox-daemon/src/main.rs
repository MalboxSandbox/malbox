use clap::Parser;
use color_eyre::Result;
use malbox_tracing::{init_tracing, parse_log_level};
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;
use tracing::{info, warn};

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

    let shutdown_token = CancellationToken::new();

    // First signal: cancel the root token for graceful shutdown.
    let signal_token = shutdown_token.clone();
    tokio::spawn(async move {
        if let Err(e) = shutdown_signal().await {
            warn!(error = %e, "Signal handler failed");
            return;
        }
        info!("Received shutdown signal, starting graceful shutdown...");
        signal_token.cancel();
    });

    // Second signal during shutdown: force exit.
    let force_token = shutdown_token.clone();
    tokio::spawn(async move {
        force_token.cancelled().await;
        if let Err(e) = shutdown_signal().await {
            warn!(error = %e, "Signal handler failed");
            return;
        }
        warn!("Received second shutdown signal, forcing exit");
        std::process::exit(1);
    });

    malbox_daemon::run(config, shutdown_token).await?;

    Ok(())
}

async fn shutdown_signal() -> std::result::Result<(), Box<dyn std::error::Error>> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm = signal(SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await?;
    }
    Ok(())
}
