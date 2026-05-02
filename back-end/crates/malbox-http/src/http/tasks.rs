pub mod cancel;
pub mod create;
pub mod get;
pub mod report;
pub mod results;

use malbox_config::Config as MalboxConfig;
use malbox_database::repositories::task_results::TaskResult;
use std::path::{Path, PathBuf};

/// Resolve the absolute on-disk path for a `task_results` row. Relative
/// `file_path`s are joined to `config.paths.data_dir` — which is where the
/// `ResultStore` (see `malbox-utils/src/storage/results.rs`) writes outputs.
pub(super) fn resolve_result_path(config: &MalboxConfig, row: &TaskResult) -> PathBuf {
    let stored = Path::new(&row.file_path);
    if stored.is_absolute() {
        stored.to_path_buf()
    } else {
        config.paths.data_dir.join(stored)
    }
}
