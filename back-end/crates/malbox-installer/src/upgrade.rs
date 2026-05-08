use crate::config::{DaemonSource, FrontendSource, UpgradeConfig};
use crate::error::{InstallError, Step};
use crate::github::{GitHubClient, Release};
use crate::manifest::Manifest;
use crate::progress::InstallProgress;
use std::path::Path;

pub async fn run(
    upgrade_config: &UpgradeConfig,
    github: &GitHubClient,
    manifest_path: &Path,
    progress: &dyn InstallProgress,
) -> crate::Result<Manifest> {
    let mut manifest = Manifest::load(manifest_path)?;

    let release = github.latest_release().await?;
    let new_version = Release::version_from_tag(&release.tag_name);

    if !upgrade_config.force && new_version == manifest.version {
        return Err(InstallError::AlreadyUpToDate(manifest.version.clone()));
    }

    // Stop daemon if systemd is configured
    if manifest.systemd.enabled
        && let Some(unit) = &manifest.systemd.unit
    {
        progress.started(Step::Systemd, "Stopping daemon");
        let _ = crate::steps::systemd::stop_service(unit).await;
        progress.completed(Step::Systemd);
    }

    // Back up current binary
    if manifest.daemon.path.exists() {
        let prev_path = manifest.daemon.path.with_extension("prev");
        tokio::fs::copy(&manifest.daemon.path, &prev_path).await?;
        manifest.daemon.prev_path = Some(prev_path);
    }

    // Determine sources - use reconfigure if provided, otherwise replay from manifest
    let (daemon_source, frontend_source) = if let Some(reconfig) = &upgrade_config.reconfigure {
        (reconfig.daemon.clone(), reconfig.frontend.clone())
    } else {
        let daemon_source = match manifest.daemon.source.as_str() {
            "prebuilt" => {
                let providers: Vec<&str> = manifest
                    .daemon
                    .providers
                    .iter()
                    .map(|s| s.as_str())
                    .collect();
                match release.find_daemon_asset(&manifest.arch, &providers) {
                    Some(asset) => DaemonSource::Prebuilt {
                        url: asset.browser_download_url.clone(),
                    },
                    None => {
                        let features: Vec<String> = manifest
                            .daemon
                            .providers
                            .iter()
                            .map(|p| format!("provider-{p}"))
                            .collect();
                        DaemonSource::Compile { features }
                    }
                }
            }
            _ => {
                let features: Vec<String> = manifest
                    .daemon
                    .providers
                    .iter()
                    .map(|p| format!("provider-{p}"))
                    .collect();
                DaemonSource::Compile { features }
            }
        };

        let frontend_source = match manifest.frontend.source.as_str() {
            "prebuilt" => match release.find_frontend_asset() {
                Some(asset) => FrontendSource::Prebuilt {
                    url: asset.browser_download_url.clone(),
                },
                None => FrontendSource::Compile,
            },
            _ => FrontendSource::Compile,
        };

        (daemon_source, frontend_source)
    };

    // Upgrade daemon
    let bin_dir = manifest.daemon.path.parent().unwrap_or(Path::new("."));
    let daemon_path =
        crate::steps::daemon::execute(&daemon_source, bin_dir, github, &release.tag_name, progress)
            .await?;
    manifest.daemon.path = daemon_path;

    // Upgrade frontend
    let data_dir = manifest.frontend.path.parent().unwrap_or(Path::new("."));
    let web_path = crate::steps::frontend::execute(
        &frontend_source,
        data_dir,
        github,
        &release.tag_name,
        progress,
    )
    .await?;
    manifest.frontend.path = web_path;

    // Run migrations
    crate::steps::postgres::execute(
        &crate::config::PostgresStrategy::Existing {
            url: manifest.postgres.url.clone(),
        },
        progress,
    )
    .await?;

    // Record upgrade
    manifest.record_upgrade(new_version, None);

    // Restart daemon if systemd is configured
    if manifest.systemd.enabled
        && let Some(unit) = &manifest.systemd.unit
    {
        progress.started(Step::Systemd, "Starting upgraded daemon");
        crate::steps::systemd::start_service(unit).await?;
        progress.completed(Step::Systemd);
    }

    manifest.save(manifest_path)?;
    Ok(manifest)
}

pub async fn rollback(manifest_path: &Path, progress: &dyn InstallProgress) -> crate::Result<()> {
    let mut manifest = Manifest::load(manifest_path)?;

    let prev_path =
        manifest.daemon.prev_path.as_ref().ok_or_else(|| {
            InstallError::Manifest("no previous binary to rollback to".to_string())
        })?;

    if !prev_path.exists() {
        return Err(InstallError::Manifest(format!(
            "previous binary not found at {}",
            prev_path.display()
        )));
    }

    // Stop daemon
    if manifest.systemd.enabled
        && let Some(unit) = &manifest.systemd.unit
    {
        let _ = crate::steps::systemd::stop_service(unit).await;
    }

    // Swap binaries
    progress.started(Step::Daemon, "Rolling back daemon binary");
    let current_path = manifest.daemon.path.clone();
    let prev_path = manifest.daemon.prev_path.take().unwrap();
    tokio::fs::copy(&prev_path, &current_path).await?;
    tokio::fs::remove_file(&prev_path).await?;
    progress.completed(Step::Daemon);

    // Restart daemon
    if manifest.systemd.enabled
        && let Some(unit) = &manifest.systemd.unit
    {
        progress.started(Step::Systemd, "Starting rolled-back daemon");
        crate::steps::systemd::start_service(unit).await?;
        progress.completed(Step::Systemd);
    }

    manifest.save(manifest_path)?;
    Ok(())
}
