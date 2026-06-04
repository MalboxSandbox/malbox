use crate::error::{Step, StepCtx};
use crate::progress::InstallProgress;
use malbox_config::{CliConfig, Config, Environment, PathConfig};
use std::path::{Path, PathBuf};

pub async fn execute(
    config_dir: &Path,
    providers: &[String],
    postgres_url: &str,
    web_dir: &Path,
    progress: &dyn InstallProgress,
) -> crate::Result<PathBuf> {
    progress.started(Step::Config, "Generating default configuration");

    tokio::fs::create_dir_all(config_dir).await?;

    let config_path = config_dir.join("malbox.toml");

    if config_path.exists() {
        progress.progress(Step::Config, 100, "Keeping existing configuration");
        progress.completed(Step::Config);
        return Ok(config_path);
    }

    // Build a typed Config and serialize it rather than templating TOML by
    // hand: the result is guaranteed to round-trip through the daemon's
    // strict (deny_unknown_fields) loader.
    let paths = PathConfig::new().step_ctx(Step::Config, "failed to resolve XDG paths")?;
    let mut config = Config::with_defaults(paths);
    config.general.environment = Environment::Production;
    config.http.web_dir = Some(web_dir.display().to_string());
    // Same-origin SPA serving needs no CORS allowances.
    config.http.cors_origins.clear();
    config.database.host = postgres_url.to_string();
    config.providers.enabled = providers.to_vec();
    config.providers.default = providers.first().cloned();

    let config_toml =
        toml::to_string_pretty(&config).step_ctx(Step::Config, "failed to serialize config")?;
    tokio::fs::write(&config_path, config_toml)
        .await
        .step_ctx(Step::Config, "failed to write config")?;

    let cli_config_path = config_dir.join("cli.toml");
    if !cli_config_path.exists() {
        let cli_toml = toml::to_string_pretty(&CliConfig::default())
            .step_ctx(Step::Config, "failed to serialize CLI config")?;
        tokio::fs::write(&cli_config_path, cli_toml).await?;
    }

    progress.completed(Step::Config);
    Ok(config_path)
}
