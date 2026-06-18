use crate::config::PostgresStrategy;
use crate::error::{InstallError, Step, StepCtx};
use crate::progress::ProgressObserver;
use std::path::{Path, PathBuf};

/// The managed instance keeps its socket in /tmp: distro packages often
/// default to /run/postgresql, which is not writable by an unprivileged
/// user-started server. The daemon itself connects over TCP on localhost.
pub(crate) const SOCKET_DIR: &str = "/tmp";

/// Connection URL for the managed instance set up by `PostgresStrategy::Setup`.
///
/// Identifies the server only - the daemon names and creates its database
/// itself. `initdb` runs as the invoking OS user, which becomes the cluster
/// superuser, so embed that identity explicitly: psql resolves it
/// implicitly, but the daemon must be told.
pub(crate) fn setup_url() -> String {
    match os_user() {
        Some(user) => format!("postgres://{user}@localhost:5432"),
        None => "postgres://localhost:5432".to_string(),
    }
}

/// The invoking OS user, mirroring libpq's default-username lookup.
fn os_user() -> Option<String> {
    ["USER", "LOGNAME"]
        .iter()
        .find_map(|var| std::env::var(var).ok().filter(|user| !user.is_empty()))
}

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

/// Check that the tools `PostgresStrategy::Setup` shells out to (initdb for
/// the cluster, psql for the connectivity check) are present. Lets the
/// wizard fail at prompt time instead of after the download steps.
pub fn detect_setup_tools() -> bool {
    detect_postgres_tools()
        && std::process::Command::new("initdb")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
}

pub async fn execute(
    strategy: &PostgresStrategy,
    data_dir: &Path,
    observer: &dyn ProgressObserver,
) -> crate::Result<PostgresResult> {
    observer.step_started("Setting up PostgreSQL");

    let result = match strategy {
        PostgresStrategy::Existing { url } => {
            observer.build_output("Testing connection to existing PostgreSQL");
            test_connection(url).await?;
            observer.build_output("Ensuring database exists");
            ensure_database(url).await?;
            PostgresResult {
                url: url.clone(),
                pgdata: None,
            }
        }
        PostgresStrategy::Setup => {
            observer.build_output("Initializing PostgreSQL cluster");
            let pgdata = data_dir.join("pgdata");
            setup_postgres(&pgdata).await?;
            observer.build_output("Testing connection");
            let url = setup_url();
            test_connection(&url).await?;
            observer.build_output("Ensuring database exists");
            ensure_database(&url).await?;
            PostgresResult {
                url,
                pgdata: Some(pgdata),
            }
        }
    };

    observer.step_completed("Setting up PostgreSQL", "");
    Ok(result)
}

/// Verify a PostgreSQL server accepts connections. The check targets the
/// `postgres` maintenance database (present in every cluster) so it works
/// before the daemon's database has been provisioned. Public so the install
/// wizard can validate user-entered URLs at prompt time instead of failing
/// minutes later.
pub async fn test_connection(url: &str) -> crate::Result<()> {
    let output = tokio::process::Command::new("psql")
        .arg(maintenance_url(url)?)
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

/// Create the daemon's database if it is missing. Database creation is
/// provisioning, so it lives here rather than in the daemon, which fails
/// with instructions when the database does not exist.
pub(crate) async fn ensure_database(url: &str) -> crate::Result<()> {
    let dbname = malbox_config::core::DATABASE_NAME;
    let maintenance = maintenance_url(url)?;

    // Probe instead of parsing CREATE DATABASE failures out of stderr:
    // psql's -tA output is stable, error text is not.
    let probe = tokio::process::Command::new("psql")
        .arg(&maintenance)
        .args(["-tA", "-c"])
        .arg(format!(
            "SELECT 1 FROM pg_database WHERE datname = '{dbname}'"
        ))
        .output()
        .await
        .step_ctx(Step::Postgres, "failed to run psql")?;

    if probe.status.success() && String::from_utf8_lossy(&probe.stdout).trim() == "1" {
        return Ok(());
    }

    let create = tokio::process::Command::new("psql")
        .arg(&maintenance)
        .arg("-c")
        .arg(format!("CREATE DATABASE \"{dbname}\""))
        .output()
        .await
        .step_ctx(Step::Postgres, "failed to run psql")?;

    if !create.status.success() {
        let stderr = String::from_utf8_lossy(&create.stderr);
        return Err(InstallError::StepFailed {
            step: Step::Postgres,
            message: format!(
                "creating database \"{dbname}\" failed: {} - create it manually \
                 with a privileged role: CREATE DATABASE \"{dbname}\"",
                stderr.trim()
            ),
        });
    }
    Ok(())
}

/// Rewrite a server URL to target the `postgres` maintenance database,
/// normalizing away any database path the user may have included.
fn maintenance_url(url: &str) -> crate::Result<String> {
    let mut parsed = url::Url::parse(url).map_err(|e| InstallError::StepFailed {
        step: Step::Postgres,
        message: format!("invalid PostgreSQL URL '{url}': {e}"),
    })?;
    parsed.set_path("/postgres");
    Ok(parsed.into())
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

    Ok(())
}
