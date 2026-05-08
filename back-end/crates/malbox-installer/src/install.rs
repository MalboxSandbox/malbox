use crate::config::{DaemonSource, FrontendSource, InstallConfig, NixStrategy, PostgresStrategy};
use crate::error::Step;
use crate::github::GitHubClient;
use crate::manifest::{
    DaemonManifest, FrontendManifest, Manifest, PostgresManifest, SystemdManifest,
};
use crate::progress::InstallProgress;
use std::path::{Path, PathBuf};

pub fn build_initial_manifest(
    config: &InstallConfig,
    version: &str,
    arch: &str,
    daemon_path: &str,
    frontend_path: &str,
) -> Manifest {
    let now = chrono::Utc::now().to_rfc3339();

    let nix_status = match config.nix {
        NixStrategy::Install => "installed",
        NixStrategy::Existing => "existing",
        NixStrategy::Skip => "skipped",
    };

    let daemon_source = match &config.daemon {
        DaemonSource::Prebuilt { .. } => "prebuilt",
        DaemonSource::Compile { .. } => "compiled",
    };

    let frontend_source = match &config.frontend {
        FrontendSource::Prebuilt { .. } => "prebuilt",
        FrontendSource::Compile => "compiled",
    };

    let (postgres_strategy, postgres_url) = match &config.postgres {
        PostgresStrategy::Existing { url } => ("existing", url.clone()),
        PostgresStrategy::Setup => ("setup", "postgres://localhost/malbox_db".to_string()),
    };

    Manifest {
        version: version.to_string(),
        schema_version: 1,
        installed_at: now.clone(),
        updated_at: now,
        arch: arch.to_string(),
        nix: nix_status.to_string(),
        daemon: DaemonManifest {
            source: daemon_source.to_string(),
            version: version.to_string(),
            commit: None,
            path: PathBuf::from(daemon_path),
            prev_path: None,
            providers: config.providers.clone(),
        },
        frontend: FrontendManifest {
            source: frontend_source.to_string(),
            path: PathBuf::from(frontend_path),
        },
        postgres: PostgresManifest {
            strategy: postgres_strategy.to_string(),
            url: postgres_url,
        },
        systemd: SystemdManifest {
            enabled: config.systemd,
            unit: if config.systemd {
                Some("malbox.service".to_string())
            } else {
                None
            },
        },
        last_completed_step: None,
    }
}

pub async fn run(
    config: &InstallConfig,
    github: &GitHubClient,
    release_tag: &str,
    manifest_path: &Path,
    progress: &dyn InstallProgress,
) -> crate::Result<Manifest> {
    let arch = current_arch();

    let bin_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .parent()
        .unwrap_or(Path::new("~/.local"))
        .join("bin");
    tokio::fs::create_dir_all(&bin_dir).await?;

    let data_dir = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("malbox");
    tokio::fs::create_dir_all(&data_dir).await?;

    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("malbox");

    let version = crate::github::Release::version_from_tag(release_tag);

    let mut manifest = build_initial_manifest(
        config,
        version,
        &arch,
        &bin_dir.join("malboxd").display().to_string(),
        &data_dir.join("web").display().to_string(),
    );

    // Step 1: Nix
    crate::steps::nix::execute(&config.nix, progress).await?;
    manifest.mark_step_completed(Step::Nix);
    manifest.save(manifest_path)?;

    // Step 2: Daemon
    let daemon_path =
        crate::steps::daemon::execute(&config.daemon, &bin_dir, github, release_tag, progress)
            .await?;
    manifest.daemon.path = daemon_path;
    manifest.mark_step_completed(Step::Daemon);
    manifest.save(manifest_path)?;

    // Step 3: Frontend
    let web_path =
        crate::steps::frontend::execute(&config.frontend, &data_dir, github, release_tag, progress)
            .await?;
    manifest.frontend.path = web_path;
    manifest.mark_step_completed(Step::Frontend);
    manifest.save(manifest_path)?;

    // Step 4: Postgres
    let pg_url = crate::steps::postgres::execute(&config.postgres, progress).await?;
    manifest.postgres.url = pg_url;
    manifest.mark_step_completed(Step::Postgres);
    manifest.save(manifest_path)?;

    // Step 5: Config
    let config_path = crate::steps::config::execute(
        &config_dir,
        &config.providers,
        &manifest.postgres.url,
        progress,
    )
    .await?;
    manifest.mark_step_completed(Step::Config);
    manifest.save(manifest_path)?;

    // Step 6: Systemd
    let unit = crate::steps::systemd::execute(
        config.systemd,
        &manifest.daemon.path,
        &config_path,
        progress,
    )
    .await?;
    manifest.systemd.unit = unit;
    manifest.mark_step_completed(Step::Systemd);

    // Mark complete
    manifest.mark_complete();
    manifest.save(manifest_path)?;

    Ok(manifest)
}

fn current_arch() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    format!("{}-unknown-{}-gnu", arch, os)
}
