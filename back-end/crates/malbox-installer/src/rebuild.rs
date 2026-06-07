//! Same-version rebuild of the installed binaries with a different Cargo
//! feature set. Backs `malboxctl provider install/uninstall/rebuild`.

use crate::config::DaemonSource;
use crate::error::Step;
use crate::github::GitHubClient;
use crate::manifest::Manifest;
use crate::progress::{InstallProgress, observe};
use std::path::Path;

/// Recompile the installed version's binaries with `features` and swap them
/// in atomically. The previous binary is kept for `upgrade --rollback`, and
/// a systemd-managed daemon is stopped/started around the swap.
pub async fn run(
    features: &[String],
    github: &GitHubClient,
    manifest_path: &Path,
    progress: &dyn InstallProgress,
) -> crate::Result<Manifest> {
    let mut manifest = Manifest::load(manifest_path)?;

    // Pin to the installed version: a rebuild changes features, not versions.
    let release = github.release_for_version(&manifest.version).await?;

    // Stop daemon if systemd is configured
    if manifest.systemd.enabled
        && let Some(unit) = &manifest.systemd.unit
    {
        progress.started(Step::Systemd, "Stopping daemon");
        let _ = crate::steps::systemd::stop_service(unit).await;
        progress.completed(Step::Systemd);
    }

    // Back up the current binary and persist the manifest immediately, so a
    // failed rebuild stays recoverable via `upgrade --rollback`.
    if manifest.daemon.path.exists() {
        let prev_path = manifest.daemon.path.with_extension("prev");
        tokio::fs::copy(&manifest.daemon.path, &prev_path).await?;
        manifest.daemon.prev_path = Some(prev_path);
        manifest.daemon.prev_version = Some(manifest.daemon.version.clone());
        manifest.save(manifest_path)?;
    }

    let bin_dir = manifest.daemon.path.parent().unwrap_or(Path::new("."));
    let result = observe(
        Step::Daemon,
        progress,
        crate::steps::daemon::execute(
            &DaemonSource::Compile,
            features,
            bin_dir,
            github,
            &release,
            progress,
        )
        .await,
    )?;
    manifest.daemon.path = result.malboxctl;
    manifest.cli.path = result.malbox;

    // Record the rebuild: the binaries are now compiled with this exact set.
    let (providers, provisioners) = crate::features::split_features(features);
    manifest.daemon.source = "compiled".to_string();
    manifest.daemon.features = features.to_vec();
    manifest.daemon.providers = providers;
    manifest.daemon.provisioners = provisioners;
    manifest.updated_at = chrono::Utc::now().to_rfc3339();

    // Restart daemon if systemd is configured
    if manifest.systemd.enabled
        && let Some(unit) = &manifest.systemd.unit
    {
        progress.started(Step::Systemd, "Starting rebuilt daemon");
        crate::steps::systemd::start_service(unit).await?;
        progress.completed(Step::Systemd);
    }

    manifest.save(manifest_path)?;
    Ok(manifest)
}
