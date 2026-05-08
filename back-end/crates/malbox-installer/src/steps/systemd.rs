use crate::error::{InstallError, Step};
use crate::progress::InstallProgress;
use std::path::Path;

const UNIT_TEMPLATE: &str = r#"[Unit]
Description=Malbox Analysis Daemon
After=network.target postgresql.service

[Service]
Type=simple
ExecStart={daemon_path}
Restart=on-failure
RestartSec=5
Environment=MALBOX_CONFIG={config_path}

[Install]
WantedBy=multi-user.target
"#;

pub async fn execute(
    enabled: bool,
    daemon_path: &Path,
    config_path: &Path,
    progress: &dyn InstallProgress,
) -> crate::Result<Option<String>> {
    if !enabled {
        return Ok(None);
    }

    progress.started(Step::Systemd, "Configuring systemd service");

    let unit_name = "malbox.service";
    let unit_content = UNIT_TEMPLATE
        .replace("{daemon_path}", &daemon_path.display().to_string())
        .replace("{config_path}", &config_path.display().to_string());

    let user_unit_dir = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("systemd")
        .join("user");

    tokio::fs::create_dir_all(&user_unit_dir).await?;

    let unit_path = user_unit_dir.join(unit_name);
    tokio::fs::write(&unit_path, &unit_content).await?;

    progress.progress(Step::Systemd, 50, "Reloading systemd daemon");
    run_systemctl(&["--user", "daemon-reload"]).await?;

    progress.progress(Step::Systemd, 75, "Enabling malbox service");
    run_systemctl(&["--user", "enable", unit_name]).await?;

    progress.completed(Step::Systemd);
    Ok(Some(unit_name.to_string()))
}

pub async fn stop_service(unit: &str) -> crate::Result<()> {
    run_systemctl(&["--user", "stop", unit]).await
}

pub async fn start_service(unit: &str) -> crate::Result<()> {
    run_systemctl(&["--user", "start", unit]).await
}

async fn run_systemctl(args: &[&str]) -> crate::Result<()> {
    let status = tokio::process::Command::new("systemctl")
        .args(args)
        .status()
        .await
        .map_err(|e| InstallError::StepFailed {
            step: Step::Systemd,
            message: format!("systemctl {} failed: {e}", args.join(" ")),
        })?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Systemd,
            message: format!("systemctl {} exited with non-zero status", args.join(" ")),
        });
    }
    Ok(())
}
