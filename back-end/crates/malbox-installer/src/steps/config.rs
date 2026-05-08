use crate::error::{InstallError, Step};
use crate::progress::InstallProgress;
use std::path::{Path, PathBuf};

pub async fn execute(
    config_dir: &Path,
    providers: &[String],
    postgres_url: &str,
    progress: &dyn InstallProgress,
) -> crate::Result<PathBuf> {
    progress.started(Step::Config, "Generating default configuration");

    tokio::fs::create_dir_all(config_dir).await?;

    let config_path = config_dir.join("malbox.toml");

    if config_path.exists() {
        progress.completed(Step::Config);
        return Ok(config_path);
    }

    let default_provider = providers.first().map(|s| s.as_str()).unwrap_or("");

    let enabled_list = providers
        .iter()
        .map(|p| format!("\"{}\"", p))
        .collect::<Vec<_>>()
        .join(", ");

    let config_content = format!(
        r#"[general]
environment = "production"
log_level = "info"
debug = false
max_workers = 4
min_workers = 1
idle_timeout_ms = 60000

[http]
host = "127.0.0.1"
port = 8080
tls_enabled = false
cors_origins = ["http://localhost:5173"]
max_upload_size = 104857600

[database]
host = "{postgres_url}"
port = 5432

[providers]
enabled = [{enabled_list}]
default = "{default_provider}"

[analysis]
timeout = 300
max_vms = 5
default_profile = "default"

[analysis.windows]
default_profile = "win10_default"
timeout = 300
max_vms = 3

[analysis.linux]
default_profile = "ubuntu_default"
timeout = 300
max_vms = 2
"#
    );

    tokio::fs::write(&config_path, config_content)
        .await
        .map_err(|e| InstallError::StepFailed {
            step: Step::Config,
            message: format!("failed to write config: {e}"),
        })?;

    let cli_config_path = config_dir.join("cli.toml");
    if !cli_config_path.exists() {
        let cli_config = "[api]\nurl = \"http://127.0.0.1:8080\"\n";
        tokio::fs::write(&cli_config_path, cli_config).await?;
    }

    progress.completed(Step::Config);
    Ok(config_path)
}
