use crate::error::{InstallError, Step, StepCtx};
use crate::progress::ProgressObserver;
use std::path::{Path, PathBuf};

const DAEMON_UNIT: &str = "malbox.service";
const POSTGRES_UNIT: &str = "malbox-postgres.service";

fn daemon_unit(daemon_path: &Path, config_path: &Path, managed_postgres: bool) -> String {
    // NOTE: a user unit cannot order against system units, so there is no
    // point referencing the distro's postgresql.service here. When the
    // installer manages its own instance we depend on that unit instead.
    let (after, wants) = if managed_postgres {
        (
            format!("network.target {POSTGRES_UNIT}"),
            format!("Wants={POSTGRES_UNIT}\n"),
        )
    } else {
        ("network.target".to_string(), String::new())
    };

    format!(
        r#"[Unit]
Description=Malbox Analysis Daemon
After={after}
{wants}
[Service]
Type=simple
ExecStart="{daemon}"
Restart=on-failure
RestartSec=5
Environment="MALBOX_CONFIG={config}"

[Install]
WantedBy=default.target
"#,
        daemon = daemon_path.display(),
        config = config_path.display(),
    )
}

fn postgres_unit(postgres_bin: &Path, pgdata: &Path) -> String {
    format!(
        r#"[Unit]
Description=Malbox Managed PostgreSQL
After=network.target

[Service]
Type=simple
ExecStart="{postgres}" -D "{pgdata}" -k {socket_dir}
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
"#,
        postgres = postgres_bin.display(),
        pgdata = pgdata.display(),
        socket_dir = super::postgres::SOCKET_DIR,
    )
}

pub async fn execute(
    enabled: bool,
    daemon_path: &Path,
    config_path: &Path,
    pgdata: Option<&Path>,
    observer: &dyn ProgressObserver,
) -> crate::Result<Option<String>> {
    observer.step_started("Configuring systemd user services");

    if !enabled {
        observer.step_completed("Configuring systemd user services", "skipped");
        return Ok(None);
    }

    let unit_dir = crate::xdg_dir(dirs::config_dir(), ".config")?
        .join("systemd")
        .join("user");
    tokio::fs::create_dir_all(&unit_dir).await?;

    if let Some(pgdata) = pgdata {
        observer.build_output("Setting up managed PostgreSQL service");
        let postgres_bin = resolve_bin("postgres").await?;
        tokio::fs::write(
            unit_dir.join(POSTGRES_UNIT),
            postgres_unit(&postgres_bin, pgdata),
        )
        .await?;
        run_systemctl(&["--user", "daemon-reload"]).await?;
        run_systemctl(&["--user", "enable", POSTGRES_UNIT]).await?;

        if super::postgres::is_running(pgdata).await {
            super::postgres::stop_instance(pgdata).await?;
        }
        run_systemctl(&["--user", "start", POSTGRES_UNIT]).await?;
    }

    observer.build_output("Installing malbox.service");
    let unit_content = daemon_unit(daemon_path, config_path, pgdata.is_some());
    tokio::fs::write(unit_dir.join(DAEMON_UNIT), unit_content).await?;

    observer.build_output("Reloading systemd daemon");
    run_systemctl(&["--user", "daemon-reload"]).await?;

    observer.build_output("Enabling malbox service");
    run_systemctl(&["--user", "enable", DAEMON_UNIT]).await?;

    observer.step_completed("Configuring systemd user services", "");
    Ok(Some(DAEMON_UNIT.to_string()))
}

pub async fn stop_service(unit: &str) -> crate::Result<()> {
    run_systemctl(&["--user", "stop", unit]).await
}

pub async fn start_service(unit: &str) -> crate::Result<()> {
    run_systemctl(&["--user", "start", unit]).await
}

/// Resolve a tool to an absolute path - systemd requires absolute
/// `ExecStart=` paths.
async fn resolve_bin(name: &str) -> crate::Result<PathBuf> {
    let output = tokio::process::Command::new("sh")
        .args(["-c", &format!("command -v {name}")])
        .output()
        .await
        .step_ctx(Step::Systemd, &format!("failed to locate {name}"))?;

    if !output.status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Systemd,
            message: format!("could not find `{name}` on PATH"),
        });
    }

    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

async fn run_systemctl(args: &[&str]) -> crate::Result<()> {
    let status = tokio::process::Command::new("systemctl")
        .args(args)
        .status()
        .await
        .step_ctx(
            Step::Systemd,
            &format!("systemctl {} failed", args.join(" ")),
        )?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Systemd,
            message: format!("systemctl {} exited with non-zero status", args.join(" ")),
        });
    }
    Ok(())
}
