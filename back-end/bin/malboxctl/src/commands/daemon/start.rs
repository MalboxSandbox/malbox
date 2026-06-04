use crate::commands::Command;
use clap::Parser;
use malbox_cli_common::context::Context;
use malbox_cli_common::error::{CliError, Result};
use malbox_tracing::parse_log_level;
use tokio_util::sync::CancellationToken;
use tracing::level_filters::LevelFilter;
use tracing::{info, warn};

/// Start the malbox daemon in the foreground.
#[derive(Parser)]
pub struct StartArgs {
    /// Log level (error | warn | info | debug | trace). Overridden by RUST_LOG.
    #[arg(long, default_value = "info", value_parser = parse_log_level)]
    pub log_level: LevelFilter,
}

impl Command for StartArgs {
    async fn execute(self, _ctx: &Context) -> Result<()> {
        // Loaded lazily (not in main) so configless commands keep working;
        // for the daemon a missing config is a hard, actionable error.
        let config = malbox_config::load_config().await.map_err(|e| match e {
            malbox_config::ConfigError::NotFound => CliError::CommandFailed(
                "no configuration found - run `malboxctl install` (or `malboxctl config init`) first"
                    .to_string(),
            ),
            e => CliError::CommandFailed(e.to_string()),
        })?;

        let shutdown_token = CancellationToken::new();

        let signal_token = shutdown_token.clone();
        tokio::spawn(async move {
            if let Err(e) = shutdown_signal().await {
                warn!(error = %e, "Signal handler failed");
                return;
            }
            info!("Received shutdown signal, starting graceful shutdown...");
            signal_token.cancel();
        });

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

        let config = config.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime for daemon");

            let result = rt.block_on(malbox_daemon::run(&config, shutdown_token));
            let _ = tx.send(result);
        });

        rx.await
            .map_err(|_| CliError::CommandFailed("Daemon thread panicked".to_string()))?
            .map_err(|e| CliError::CommandFailed(e.to_string()))
    }
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
