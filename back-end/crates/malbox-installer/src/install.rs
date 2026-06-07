use crate::config::{DaemonSource, FrontendSource, InstallConfig, PostgresStrategy};
use crate::error::{InstallError, Step};
use crate::github::{GitHubClient, Release};
use crate::manifest::{
    CliManifest, DaemonManifest, FrontendManifest, Manifest, PostgresManifest, SystemdManifest,
};
use crate::progress::{InstallProgress, observe};
use std::path::{Path, PathBuf};

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

    let manifest = build_initial_manifest(
        config,
        release.version(),
        &arch_label(),
        &bin_dir.join("malboxctl").display().to_string(),
        &bin_dir.join("malbox").display().to_string(),
        &data_dir.join("web").display().to_string(),
    );

    let plan = StepPlan {
        manifest,
        resume_after: None,
        bin_dir,
        data_dir,
        config_dir,
    };
    run_steps(config, github, release, manifest_path, progress, plan).await
}

/// Continue an interrupted installation from the step after the last
/// completed one, re-deriving the installation choices recorded in the
/// manifest. Pins to the release the interrupted install was installing
/// rather than floating to the latest.
pub async fn resume(
    github: &GitHubClient,
    manifest_path: &Path,
    progress: &dyn InstallProgress,
) -> crate::Result<Manifest> {
    let manifest = Manifest::load(manifest_path)?;
    let Some(last_completed) = manifest.last_completed_step else {
        return Err(InstallError::Manifest(
            "nothing to resume - the installation is complete".to_string(),
        ));
    };

    let release = github.release_for_version(&manifest.version).await?;
    let config = config_from_manifest(&manifest, &release);

    // The initial manifest records the intended paths up front, so the
    // install directories are recoverable no matter where the install died.
    let bin_dir = parent_of(&manifest.daemon.path);
    let data_dir = parent_of(&manifest.frontend.path);
    let config_dir = parent_of(manifest_path);

    let plan = StepPlan {
        manifest,
        resume_after: Some(last_completed),
        bin_dir,
        data_dir,
        config_dir,
    };
    run_steps(&config, github, &release, manifest_path, progress, plan).await
}

struct StepPlan {
    manifest: Manifest,
    /// Skip steps up to and including this one (resume).
    resume_after: Option<Step>,
    bin_dir: PathBuf,
    data_dir: PathBuf,
    config_dir: PathBuf,
}

async fn run_steps(
    config: &InstallConfig,
    github: &GitHubClient,
    release: &Release,
    manifest_path: &Path,
    progress: &dyn InstallProgress,
    plan: StepPlan,
) -> crate::Result<Manifest> {
    let StepPlan {
        mut manifest,
        resume_after,
        bin_dir,
        data_dir,
        config_dir,
    } = plan;
    let should_run = |step: Step| resume_after.is_none_or(|last| step.is_after(last));

    // Step 1: Binaries (malboxctl + malbox)
    if should_run(Step::Daemon) {
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
    }

    // Step 2: Frontend
    if should_run(Step::Frontend) {
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
    }

    // Step 3: Postgres
    let pgdata = if should_run(Step::Postgres) {
        let pg = observe(
            Step::Postgres,
            progress,
            crate::steps::postgres::execute(&config.postgres, &data_dir, progress).await,
        )?;
        manifest.postgres.url = pg.url;
        manifest.mark_step_completed(Step::Postgres);
        manifest.save(manifest_path)?;
        pg.pgdata
    } else {
        // Set up on a previous run; recover the managed instance's data
        // directory for the systemd step.
        (manifest.postgres.strategy == "setup").then(|| data_dir.join("pgdata"))
    };

    // Step 4: Config
    let config_path = config_dir.join("malbox.toml");
    if should_run(Step::Config) {
        observe(
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
    }

    // Step 5: Systemd
    let unit = observe(
        Step::Systemd,
        progress,
        crate::steps::systemd::execute(
            config.systemd,
            &manifest.daemon.path,
            &config_path,
            pgdata.as_deref(),
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

/// Reconstruct the installation choices recorded in an interrupted install's
/// manifest, re-deriving release-dependent download URLs the same way the
/// wizard does.
fn config_from_manifest(manifest: &Manifest, release: &Release) -> InstallConfig {
    // "prebuilt" was a user choice; honor it when the release still has the
    // asset. "compiled" stays compiled.
    let daemon = match (
        manifest.daemon.source.as_str(),
        crate::steps::daemon::release_arch(),
    ) {
        ("prebuilt", Some(arch)) => match release.find_malboxctl_asset(arch) {
            Some(asset) => DaemonSource::Prebuilt {
                url: asset.browser_download_url.clone(),
            },
            None => DaemonSource::Compile,
        },
        _ => DaemonSource::Compile,
    };

    let frontend = match release.find_web_asset() {
        Some(asset) => FrontendSource::Prebuilt {
            url: asset.browser_download_url.clone(),
        },
        None => FrontendSource::Compile,
    };

    let postgres = match manifest.postgres.strategy.as_str() {
        "existing" => PostgresStrategy::Existing {
            url: manifest.postgres.url.clone(),
        },
        _ => PostgresStrategy::Setup,
    };

    let features = if manifest.daemon.features.is_empty() {
        crate::features::default_features()
    } else {
        manifest.daemon.features.clone()
    };

    InstallConfig {
        providers: manifest.daemon.providers.clone(),
        provisioners: manifest.daemon.provisioners.clone(),
        features,
        channel: manifest.channel,
        daemon,
        frontend,
        postgres,
        systemd: manifest.systemd.enabled,
    }
}

fn parent_of(path: &Path) -> PathBuf {
    path.parent().unwrap_or(Path::new(".")).to_path_buf()
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
