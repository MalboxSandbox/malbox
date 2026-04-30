//! Auto-collection of artifact and external log files after task execution.
//!
//! After `HostPlugin::on_task` returns, the runtime walks the artifact and
//! external-log directories and sends any files that were not already
//! explicitly sent (or marked as collected) by the plugin.

use crate::context::Context;
use crate::types::PluginResult;
use globset::{Glob, GlobSetBuilder};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// Resolved auto-collection settings for a single directory.
#[derive(Debug, Clone)]
pub(crate) struct AutoCollectSection {
    pub enabled: bool,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub max_file_size: u64,
}

/// Collect files from `dir` and send them as results via `ctx.push_result`.
///
/// When `claimed_paths` is `Some`, files whose canonical path appears in the
/// set are skipped (artifact dedup). When `None`, all matching files are sent
/// (external log collection - no dedup).
pub(crate) fn auto_collect(
    ctx: &Context<'_>,
    dir: &Path,
    config: &AutoCollectSection,
    claimed_paths: Option<&HashSet<PathBuf>>,
    result_prefix: &str,
) {
    if !config.enabled {
        return;
    }

    if !dir.exists() {
        debug!(dir = %dir.display(), "auto-collect directory does not exist, skipping");
        return;
    }

    let include_set = match build_globset(&config.include) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to compile auto-collect include patterns, skipping");
            return;
        }
    };

    let exclude_set = match build_globset(&config.exclude) {
        Ok(s) => s,
        Err(e) => {
            warn!(error = %e, "failed to compile auto-collect exclude patterns, skipping");
            return;
        }
    };

    let entries = match collect_files(dir) {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, dir = %dir.display(), "failed to walk auto-collect directory");
            return;
        }
    };

    for file_path in entries {
        let relative = match file_path.strip_prefix(dir) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let rel_str = relative.to_string_lossy().into_owned();

        if !include_set.is_match(relative) {
            continue;
        }
        if exclude_set.is_match(relative) {
            continue;
        }

        if let Some(claimed) = claimed_paths {
            if let Ok(canonical) = std::fs::canonicalize(&file_path) {
                if claimed.contains(&canonical) {
                    debug!(path = %rel_str, "skipping already-claimed artifact");
                    continue;
                }
            }
        }

        let size = match std::fs::metadata(&file_path) {
            Ok(m) => m.len(),
            Err(e) => {
                warn!(path = %rel_str, error = %e, "failed to stat file, skipping");
                continue;
            }
        };

        if size > config.max_file_size {
            debug!(
                path = %rel_str,
                size,
                max = config.max_file_size,
                "file exceeds max_file_size, skipping"
            );
            continue;
        }

        let result_name = if result_prefix.is_empty() {
            rel_str.to_string()
        } else {
            format!("{result_prefix}/{rel_str}")
        };

        if let Err(e) = ctx.push_result(PluginResult::file(result_name.clone(), &file_path)) {
            warn!(
                path = %file_path.display(),
                error = %e,
                "failed to auto-collect file"
            );
        } else {
            debug!(result_name, "auto-collected file");
        }
    }
}

fn build_globset(patterns: &[String]) -> std::result::Result<globset::GlobSet, globset::Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    builder.build()
}

fn collect_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}
