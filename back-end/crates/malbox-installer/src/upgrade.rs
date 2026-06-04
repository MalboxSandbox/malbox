use crate::config::{DaemonSource, FrontendSource, UpgradeConfig};
use crate::error::{InstallError, Step};
use crate::github::GitHubClient;
use crate::manifest::Manifest;
use crate::progress::{InstallProgress, observe};
use crate::steps::daemon::release_arch;
use std::path::Path;

pub async fn run(
    upgrade_config: &UpgradeConfig,
    github: &GitHubClient,
    manifest_path: &Path,
    progress: &dyn InstallProgress,
) -> crate::Result<Manifest> {
    let mut manifest = Manifest::load(manifest_path)?;

    let release = github.latest_release(manifest.channel).await?;
    let new_version = release.version();

    if !upgrade_config.force && !is_upgrade(&manifest.version, new_version) {
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

    // Back up the current binary and persist the manifest immediately: if
    // the upgrade dies halfway, the on-disk manifest must already know about
    // the backup for `upgrade --rollback` to find it.
    if manifest.daemon.path.exists() {
        let prev_path = manifest.daemon.path.with_extension("prev");
        tokio::fs::copy(&manifest.daemon.path, &prev_path).await?;
        manifest.daemon.prev_path = Some(prev_path);
        manifest.daemon.prev_version = Some(manifest.daemon.version.clone());
        manifest.save(manifest_path)?;
    }

    // Determine sources - reconfigure wins, otherwise replay from the manifest.
    let (daemon_source, features, frontend_source) =
        if let Some(reconfig) = &upgrade_config.reconfigure {
            (
                reconfig.daemon.clone(),
                reconfig.features.clone(),
                reconfig.frontend.clone(),
            )
        } else {
            let prebuilt_asset = (manifest.daemon.source == "prebuilt")
                .then(release_arch)
                .flatten()
                .and_then(|arch| release.find_malboxctl_asset(arch));

            let daemon_source = match prebuilt_asset {
                Some(asset) => DaemonSource::Prebuilt {
                    url: asset.browser_download_url.clone(),
                },
                None => DaemonSource::Compile,
            };

            // Replay the exact cargo feature set recorded at install time.
            // Manifests that predate feature recording fall back to the
            // default set.
            let features = if manifest.daemon.features.is_empty() {
                crate::DEFAULT_DAEMON_FEATURES
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            } else {
                manifest.daemon.features.clone()
            };

            let frontend_source = match release.find_web_asset() {
                Some(asset) => FrontendSource::Prebuilt {
                    url: asset.browser_download_url.clone(),
                },
                None => FrontendSource::Compile,
            };

            (daemon_source, features, frontend_source)
        };

    // Upgrade binaries
    let bin_dir = manifest.daemon.path.parent().unwrap_or(Path::new("."));
    let result = observe(
        Step::Daemon,
        progress,
        crate::steps::daemon::execute(
            &daemon_source,
            &features,
            bin_dir,
            github,
            &release,
            progress,
        )
        .await,
    )?;
    manifest.daemon.path = result.malboxctl;
    manifest.cli.path = result.malbox;

    // Upgrade frontend
    let data_dir = manifest
        .frontend
        .path
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let frontend = observe(
        Step::Frontend,
        progress,
        crate::steps::frontend::execute(&frontend_source, &data_dir, github, &release, progress)
            .await,
    )?;
    manifest.frontend.path = frontend.path;
    manifest.frontend.prev_path = frontend.prev_path;

    // Connectivity check only - the upgraded daemon migrates its own schema
    // at startup.
    observe(
        Step::Postgres,
        progress,
        crate::steps::postgres::execute(
            &crate::config::PostgresStrategy::Existing {
                url: manifest.postgres.url.clone(),
            },
            &data_dir,
            progress,
        )
        .await,
    )?;

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

/// True when `remote` is strictly newer than `local`. Falls back to plain
/// inequality when either side is not valid semver (custom tags), matching
/// the historical behavior.
fn is_upgrade(local: &str, remote: &str) -> bool {
    match (
        semver::Version::parse(local),
        semver::Version::parse(remote),
    ) {
        (Ok(local), Ok(remote)) => remote > local,
        _ => local != remote,
    }
}

pub async fn rollback(manifest_path: &Path, progress: &dyn InstallProgress) -> crate::Result<()> {
    let mut manifest = Manifest::load(manifest_path)?;

    let prev_path =
        manifest.daemon.prev_path.clone().ok_or_else(|| {
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

    // Swap binaries. Rename, not copy: atomic, and immune to ETXTBSY when
    // malboxctl is rolling itself back.
    progress.started(Step::Daemon, "Rolling back malbox binaries");
    tokio::fs::rename(&prev_path, &manifest.daemon.path).await?;
    manifest.daemon.prev_path = None;
    if let Some(prev_version) = manifest.daemon.prev_version.take() {
        manifest.version = prev_version.clone();
        manifest.daemon.version = prev_version;
    }
    progress.completed(Step::Daemon);

    // Restore the previous front-end bundle when one was kept.
    if let Some(web_prev) = manifest.frontend.prev_path.take()
        && web_prev.exists()
    {
        progress.started(Step::Frontend, "Rolling back front-end assets");
        if manifest.frontend.path.exists() {
            tokio::fs::remove_dir_all(&manifest.frontend.path).await?;
        }
        tokio::fs::rename(&web_prev, &manifest.frontend.path).await?;
        progress.completed(Step::Frontend);
    }

    manifest.updated_at = chrono::Utc::now().to_rfc3339();

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
