use crate::error::{InstallError, Step, StepCtx};
use crate::progress::ProgressObserver;
use malbox_config::core::{DATABASE_NAME, DatabaseConfig};
use malbox_config::{CliConfig, Config, Environment, PathConfig};
use std::path::{Path, PathBuf};

pub async fn execute(
    config_dir: &Path,
    providers: &[String],
    postgres_url: &str,
    web_dir: &Path,
    observer: &dyn ProgressObserver,
) -> crate::Result<PathBuf> {
    observer.step_started("Generating default configuration");

    tokio::fs::create_dir_all(config_dir).await?;

    let config_path = config_dir.join("malbox.toml");

    if config_path.exists() {
        observer.build_output("Keeping existing configuration");
        observer.step_completed(
            "Generating default configuration",
            "existing config preserved",
        );
        return Ok(config_path);
    }

    let paths = PathConfig::new().step_ctx(Step::Config, "failed to resolve XDG paths")?;
    let mut config = Config::with_defaults(paths);
    config.general.environment = Environment::Production;
    config.http.web_dir = Some(web_dir.display().to_string());
    config.http.cors_origins.clear();
    config.database = database_config(postgres_url)?;
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

    observer.step_completed("Generating default configuration", "");
    Ok(config_path)
}

fn database_config(url: &str) -> crate::Result<DatabaseConfig> {
    let parsed = url::Url::parse(url).map_err(|e| invalid_url(url, &e.to_string()))?;

    if !matches!(parsed.scheme(), "postgres" | "postgresql") {
        return Err(invalid_url(url, "expected a postgres:// URL"));
    }
    if parsed.query().is_some() {
        return Err(invalid_url(
            url,
            "query parameters are not supported; TLS is negotiated automatically",
        ));
    }
    let path = parsed.path().trim_start_matches('/');
    if !path.is_empty() && path != DATABASE_NAME {
        return Err(invalid_url(
            url,
            &format!(
                "the database name is not configurable; the daemon always uses \
                 \"{DATABASE_NAME}\" - drop the '/{path}' suffix"
            ),
        ));
    }

    let mut database = DatabaseConfig::default();
    if let Some(host) = parsed.host_str() {
        database.host = host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_string();
    }
    if let Some(port) = parsed.port() {
        database.port = port;
    }
    if !parsed.username().is_empty() {
        database.user = Some(parsed.username().to_string());
    }
    database.password = parsed.password().map(str::to_string);

    Ok(database)
}

fn invalid_url(url: &str, reason: &str) -> InstallError {
    InstallError::StepFailed {
        step: Step::Config,
        message: format!("invalid PostgreSQL URL '{url}': {reason}"),
    }
}
