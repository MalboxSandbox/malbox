use crate::error::{InstallError, Step, StepCtx};
use crate::progress::ProgressObserver;
use std::path::{Path, PathBuf};

const DAEMON_UNIT: &str = "malbox.service";
const POSTGRES_UNIT: &str = "malbox-postgres.service";

/// Whether the systemd step should install units or only report itself as skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnitInstall {
    /// Write, reload, and enable the user units.
    Install,
    /// Leave systemd untouched.
    Skip,
}

impl From<bool> for UnitInstall {
    fn from(enabled: bool) -> Self {
        if enabled { Self::Install } else { Self::Skip }
    }
}

/// Renders the daemon unit; `pgdata` is set when the installer manages `PostgreSQL`.
fn daemon_unit(daemon_path: &Path, config_path: &Path, pgdata: Option<&Path>) -> String {
    // NOTE: a user unit cannot order against system units, so there is no
    // point referencing the distro's postgresql.service here. When the
    // installer manages its own instance we depend on that unit instead.
    let (after, wants) = if pgdata.is_some() {
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

/// Installs and enables the malbox (and, with `pgdata`, managed `PostgreSQL`) user units.
///
/// Returns the daemon unit name, or `None` when `mode` is [`UnitInstall::Skip`].
///
/// # Errors
///
/// Returns [`InstallError::Environment`] when the home directory is unknown,
/// [`InstallError::Io`] when unit files cannot be written, and
/// [`InstallError::StepFailed`] when a required binary or `systemctl` fails.
pub async fn execute(
    mode: UnitInstall,
    daemon_path: &Path,
    config_path: &Path,
    pgdata: Option<&Path>,
    observer: &dyn ProgressObserver,
) -> crate::Result<Option<String>> {
    observer.step_started("Configuring systemd user services");

    if mode == UnitInstall::Skip {
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
    let unit_content = daemon_unit(daemon_path, config_path, pgdata);
    tokio::fs::write(unit_dir.join(DAEMON_UNIT), unit_content).await?;

    observer.build_output("Reloading systemd daemon");
    run_systemctl(&["--user", "daemon-reload"]).await?;

    observer.build_output("Enabling malbox service");
    run_systemctl(&["--user", "enable", DAEMON_UNIT]).await?;

    observer.step_completed("Configuring systemd user services", "");
    Ok(Some(DAEMON_UNIT.to_string()))
}

/// Stops the given systemd user unit.
///
/// # Errors
///
/// Returns [`InstallError::StepFailed`] when `systemctl` cannot run or reports failure.
pub async fn stop_service(unit: &str) -> crate::Result<()> {
    run_systemctl(&["--user", "stop", unit]).await
}

/// Starts the given systemd user unit.
///
/// # Errors
///
/// Returns [`InstallError::StepFailed`] when `systemctl` cannot run or reports failure.
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
