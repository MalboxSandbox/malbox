use crate::config::PostgresStrategy;
use crate::error::{InstallError, Step, StepCtx};
use crate::progress::InstallProgress;
use std::path::{Path, PathBuf};

/// Connection URL for the managed instance set up by `PostgresStrategy::Setup`.
const SETUP_URL: &str = "postgres://localhost/malbox_db";

/// The managed instance keeps its socket in /tmp: distro packages often
/// default to /run/postgresql, which is not writable by an unprivileged
/// user-started server. The daemon itself connects over TCP on localhost.
pub(crate) const SOCKET_DIR: &str = "/tmp";

pub struct PostgresResult {
    pub url: String,
    /// Data directory of the managed instance, when `Setup` ran. The systemd
    /// step uses it to generate a unit that owns the instance across reboots.
    pub pgdata: Option<PathBuf>,
}

/// Best-effort check that PostgreSQL *client tools* are installed. Says
/// nothing about a server actually running - prompts should be worded
/// accordingly.
pub fn detect_postgres_tools() -> bool {
    std::process::Command::new("psql")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

pub async fn execute(
    strategy: &PostgresStrategy,
    data_dir: &Path,
    progress: &dyn InstallProgress,
) -> crate::Result<PostgresResult> {
    progress.started(Step::Postgres, "Setting up PostgreSQL");

    let result = match strategy {
        PostgresStrategy::Existing { url } => {
            progress.progress(
                Step::Postgres,
                50,
                "Testing connection to existing PostgreSQL",
            );
            test_connection(url).await?;
            PostgresResult {
                url: url.clone(),
                pgdata: None,
            }
        }
        PostgresStrategy::Setup => {
            progress.progress(Step::Postgres, 10, "Setting up PostgreSQL");
            let pgdata = data_dir.join("pgdata");
            setup_postgres(&pgdata).await?;
            progress.progress(Step::Postgres, 70, "Testing connection");
            test_connection(SETUP_URL).await?;
            PostgresResult {
                url: SETUP_URL.to_string(),
                pgdata: Some(pgdata),
            }
        }
    };

    // NOTE: schema migrations are deliberately not run here. They are
    // embedded in the daemon (sqlx::migrate! in malbox-database) and applied
    // automatically at startup, which keeps them correct across upgrades.
    progress.completed(Step::Postgres);
    Ok(result)
}

/// Verify a PostgreSQL URL accepts connections. Public so the install wizard
/// can validate user-entered URLs at prompt time instead of failing minutes
/// later.
pub async fn test_connection(url: &str) -> crate::Result<()> {
    let output = tokio::process::Command::new("psql")
        .arg(url)
        .arg("-c")
        .arg("SELECT 1")
        .output()
        .await
        .step_ctx(Step::Postgres, "failed to run psql")?;

    if !output.status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Postgres,
            message: format!("PostgreSQL connection failed at {url} - is the service running?"),
        });
    }
    Ok(())
}

/// True when the managed instance at `pgdata` is currently running.
pub(crate) async fn is_running(pgdata: &Path) -> bool {
    tokio::process::Command::new("pg_ctl")
        .arg("-D")
        .arg(pgdata)
        .arg("status")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

/// Stop the managed instance at `pgdata` (used when handing it over to a
/// systemd unit).
pub(crate) async fn stop_instance(pgdata: &Path) -> crate::Result<()> {
    let status = tokio::process::Command::new("pg_ctl")
        .arg("-D")
        .arg(pgdata)
        .args(["-m", "fast", "stop"])
        .status()
        .await
        .step_ctx(Step::Systemd, "failed to run pg_ctl stop")?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Systemd,
            message: "pg_ctl stop failed".to_string(),
        });
    }
    Ok(())
}

/// Initialize (idempotently) and start a user-owned PostgreSQL instance.
async fn setup_postgres(pgdata: &Path) -> crate::Result<()> {
    crate::steps::ensure_tool(
        "initdb",
        "install PostgreSQL via your system package manager first",
        Step::Postgres,
    )
    .await?;

    tokio::fs::create_dir_all(pgdata).await?;

    // A populated data directory means a previous run (or install) already
    // initialized the cluster; initdb would refuse to run again.
    if !pgdata.join("PG_VERSION").exists() {
        let status = tokio::process::Command::new("initdb")
            .arg("-D")
            .arg(pgdata)
            .status()
            .await
            .step_ctx(Step::Postgres, "initdb failed")?;

        if !status.success() {
            return Err(InstallError::StepFailed {
                step: Step::Postgres,
                message: "initdb failed".to_string(),
            });
        }
    }

    if !is_running(pgdata).await {
        let log_path = pgdata
            .parent()
            .unwrap_or(Path::new("."))
            .join("postgres.log");
        let status = tokio::process::Command::new("pg_ctl")
            .arg("-D")
            .arg(pgdata)
            .arg("-l")
            .arg(&log_path)
            .args(["-o", &format!("-k {SOCKET_DIR}")])
            .arg("start")
            .status()
            .await
            .step_ctx(Step::Postgres, "pg_ctl start failed")?;

        if !status.success() {
            return Err(InstallError::StepFailed {
                step: Step::Postgres,
                message: format!(
                    "pg_ctl start failed (see {}) - is another PostgreSQL instance already using the default port?",
                    log_path.display()
                ),
            });
        }
    }

    create_database().await
}

/// Create the malbox database, tolerating an existing one from a previous
/// install.
async fn create_database() -> crate::Result<()> {
    let output = tokio::process::Command::new("createdb")
        .args(["-h", SOCKET_DIR, "malbox_db"])
        .output()
        .await
        .step_ctx(Step::Postgres, "createdb failed")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("already exists") {
        return Ok(());
    }

    Err(InstallError::StepFailed {
        step: Step::Postgres,
        message: format!("createdb malbox_db failed: {}", stderr.trim()),
    })
}
