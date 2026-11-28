use crate::error::{RegistryError, Result};
use crate::lockfile::Lockfile;
use std::path::Path;

/// Delete an installed plugin's directory and drop its lockfile entry.
///
/// # Errors
///
/// Returns [`RegistryError::InvalidSpecifier`] for an unsafe name,
/// [`RegistryError::NotInstalled`] if the directory is missing,
/// [`RegistryError::InvalidPlugin`] if it resolves outside `plugins_dir`, and
/// I/O or lockfile errors from deletion and the lockfile update.
pub fn remove_plugin(name: &str, plugins_dir: &Path) -> Result<()> {
    crate::error::validate_plugin_name(name)?;

    let target_dir = plugins_dir.join(name);

    if !target_dir.exists() {
        return Err(RegistryError::NotInstalled(name.to_string()));
    }

    let canonical = target_dir.canonicalize()?;
    let base = plugins_dir.canonicalize()?;
    if !canonical.starts_with(&base) || canonical == base {
        return Err(RegistryError::InvalidPlugin {
            path: target_dir,
            reason: "path traversal detected".into(),
        });
    }

    std::fs::remove_dir_all(&canonical)?;

    let lockfile_path = Lockfile::lockfile_path(plugins_dir);
    let mut lockfile = Lockfile::load(&lockfile_path)?;
    lockfile.plugins.remove(name);
    lockfile.write(&lockfile_path)?;

    Ok(())
}
