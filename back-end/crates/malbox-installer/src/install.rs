use crate::config::{DaemonSource, FrontendSource, InstallConfig, PostgresStrategy};
use crate::error::Step;
use crate::github::{GitHubClient, Release};
use crate::manifest::{
    CliManifest, DaemonManifest, FrontendManifest, Manifest, PostgresManifest, SystemdManifest,
};
use crate::progress::{InstallProgress, observe};
use std::path::PathBuf;

pub fn build_initial_manifest(
    config: &InstallConfig,
    version: &str,
    arch: &str,
    malboxctl_path: &str,
    malbox_path: &str,
    frontend_path: &str,
) -> Manifest {
    let now = chrono::Utc::now().to_rfc3339();

    let daemon_source = match &config.daemon {
        DaemonSource::Prebuilt { .. } => "prebuilt",
        DaemonSource::Compile => "compiled",
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
        channel: config.channel,
        daemon: DaemonManifest {
            source: daemon_source.to_string(),
            version: version.to_string(),
            commit: None,
            path: PathBuf::from(malboxctl_path),
            prev_path: None,
            prev_version: None,
            providers: config.providers.clone(),
            provisioners: config.provisioners.clone(),
            features: config.features.clone(),
        },
        frontend: FrontendManifest {
            source: frontend_source.to_string(),
            path: PathBuf::from(frontend_path),
            prev_path: None,
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
        cli: CliManifest {
            path: PathBuf::from(malbox_path),
        },
        last_completed_step: None,
    }
}

pub async fn run(
    config: &InstallConfig,
    github: &GitHubClient,
    release: &Release,
    manifest_path: &std::path::Path,
    progress: &dyn InstallProgress,
) -> crate::Result<Manifest> {
    let bin_dir = crate::xdg_dir(dirs::executable_dir(), ".local/bin")?;
    tokio::fs::create_dir_all(&bin_dir).await?;

    let data_dir = crate::xdg_dir(dirs::data_local_dir(), ".local/share")?.join("malbox");
    tokio::fs::create_dir_all(&data_dir).await?;

    let config_dir = crate::xdg_dir(dirs::config_dir(), ".config")?.join("malbox");

    let mut manifest = build_initial_manifest(
        config,
        release.version(),
        &arch_label(),
        &bin_dir.join("malboxctl").display().to_string(),
        &bin_dir.join("malbox").display().to_string(),
        &data_dir.join("web").display().to_string(),
    );

    // Step 1: Binaries (malboxctl + malbox)
    let result = observe(
        Step::Daemon,
        progress,
        crate::steps::daemon::execute(
            &config.daemon,
            &config.features,
            &bin_dir,
            github,
            release,
            progress,
        )
        .await,
    )?;
    manifest.daemon.path = result.malboxctl;
    manifest.cli.path = result.malbox;
    manifest.mark_step_completed(Step::Daemon);
    manifest.save(manifest_path)?;

    // Step 2: Frontend
    let frontend = observe(
        Step::Frontend,
        progress,
        crate::steps::frontend::execute(&config.frontend, &data_dir, github, release, progress)
            .await,
    )?;
    manifest.frontend.path = frontend.path;
    manifest.frontend.prev_path = frontend.prev_path;
    manifest.mark_step_completed(Step::Frontend);
    manifest.save(manifest_path)?;

    // Step 3: Postgres
    let pg = observe(
        Step::Postgres,
        progress,
        crate::steps::postgres::execute(&config.postgres, &data_dir, progress).await,
    )?;
    manifest.postgres.url = pg.url;
    manifest.mark_step_completed(Step::Postgres);
    manifest.save(manifest_path)?;

    // Step 4: Config
    let config_path = observe(
        Step::Config,
        progress,
        crate::steps::config::execute(
            &config_dir,
            &config.providers,
            &manifest.postgres.url,
            &manifest.frontend.path,
            progress,
        )
        .await,
    )?;
    manifest.mark_step_completed(Step::Config);
    manifest.save(manifest_path)?;

    // Step 5: Systemd
    let unit = observe(
        Step::Systemd,
        progress,
        crate::steps::systemd::execute(
            config.systemd,
            &manifest.daemon.path,
            &config_path,
            pg.pgdata.as_deref(),
            progress,
        )
        .await,
    )?;
    manifest.systemd.unit = unit;
    manifest.mark_step_completed(Step::Systemd);

    // Mark complete
    manifest.mark_complete();
    manifest.save(manifest_path)?;

    Ok(manifest)
}

/// Architecture label recorded in the manifest. Uses the release asset
/// vocabulary (`linux-x64`) so the manifest speaks the same language as the
/// artifacts it describes; platforms without prebuilt assets fall back to a
/// plain `arch-os` pair.
fn arch_label() -> String {
    match crate::steps::daemon::release_arch() {
        Some(arch) => arch.to_string(),
        None => format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS),
    }
}
