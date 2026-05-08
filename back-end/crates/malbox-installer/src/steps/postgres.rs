use crate::config::PostgresStrategy;
use crate::error::{InstallError, Step};
use crate::progress::InstallProgress;

pub fn detect_postgres() -> bool {
    std::process::Command::new("psql")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

pub async fn execute(
    strategy: &PostgresStrategy,
    progress: &dyn InstallProgress,
) -> crate::Result<String> {
    progress.started(Step::Postgres, "Setting up PostgreSQL");

    let url = match strategy {
        PostgresStrategy::Existing { url } => {
            progress.progress(
                Step::Postgres,
                50,
                "Testing connection to existing PostgreSQL",
            );
            test_connection(url).await?;
            url.clone()
        }
        PostgresStrategy::Setup => {
            progress.progress(Step::Postgres, 10, "Installing PostgreSQL via Nix");
            let url = setup_postgres().await?;
            progress.progress(Step::Postgres, 70, "Testing connection");
            test_connection(&url).await?;
            url
        }
    };

    progress.progress(Step::Postgres, 90, "Running database migrations");
    run_migrations(&url).await?;

    progress.completed(Step::Postgres);
    Ok(url)
}

async fn test_connection(url: &str) -> crate::Result<()> {
    let status = tokio::process::Command::new("psql")
        .arg(url)
        .arg("-c")
        .arg("SELECT 1")
        .output()
        .await
        .map_err(|e| InstallError::StepFailed {
            step: Step::Postgres,
            message: format!("failed to run psql: {e}"),
        })?;

    if !status.status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Postgres,
            message: format!("PostgreSQL connection failed at {url} - is the service running?"),
        });
    }
    Ok(())
}

async fn setup_postgres() -> crate::Result<String> {
    let status = tokio::process::Command::new("nix")
        .args(["profile", "install", "nixpkgs#postgresql_16"])
        .status()
        .await
        .map_err(|e| InstallError::StepFailed {
            step: Step::Postgres,
            message: format!("failed to install PostgreSQL via Nix: {e}"),
        })?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Postgres,
            message: "nix profile install postgresql failed".to_string(),
        });
    }

    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.local/share"))
        .join("malbox")
        .join("pgdata");

    tokio::fs::create_dir_all(&data_dir).await?;

    let init_status = tokio::process::Command::new("initdb")
        .arg("-D")
        .arg(&data_dir)
        .status()
        .await
        .map_err(|e| InstallError::StepFailed {
            step: Step::Postgres,
            message: format!("initdb failed: {e}"),
        })?;

    if !init_status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Postgres,
            message: "initdb failed".to_string(),
        });
    }

    let pg_status = tokio::process::Command::new("pg_ctl")
        .arg("-D")
        .arg(&data_dir)
        .arg("-l")
        .arg(data_dir.parent().unwrap().join("postgres.log"))
        .arg("start")
        .status()
        .await
        .map_err(|e| InstallError::StepFailed {
            step: Step::Postgres,
            message: format!("pg_ctl start failed: {e}"),
        })?;

    if !pg_status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Postgres,
            message: "pg_ctl start failed".to_string(),
        });
    }

    let createdb = tokio::process::Command::new("createdb")
        .arg("malbox_db")
        .status()
        .await
        .map_err(|e| InstallError::StepFailed {
            step: Step::Postgres,
            message: format!("createdb failed: {e}"),
        })?;

    if !createdb.success() {
        return Err(InstallError::StepFailed {
            step: Step::Postgres,
            message: "createdb malbox_db failed".to_string(),
        });
    }

    Ok("postgres://localhost/malbox_db".to_string())
}

async fn run_migrations(url: &str) -> crate::Result<()> {
    let status = tokio::process::Command::new("sqlx")
        .args(["migrate", "run"])
        .env("DATABASE_URL", url)
        .status()
        .await
        .map_err(|e| InstallError::StepFailed {
            step: Step::Postgres,
            message: format!("failed to run migrations: {e}"),
        })?;

    if !status.success() {
        return Err(InstallError::StepFailed {
            step: Step::Postgres,
            message: "database migrations failed".to_string(),
        });
    }
    Ok(())
}
