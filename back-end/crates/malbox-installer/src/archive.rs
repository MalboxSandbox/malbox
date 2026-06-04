use crate::error::{InstallError, Step, StepCtx};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(crate) fn extract_tarball(bytes: &[u8], dest: &Path, step: Step) -> crate::Result<()> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(dest)
        .step_ctx(step, "failed to extract tarball")
}

/// Write `reader` to `dest` as an executable via a temp file in the same
/// directory followed by an atomic rename. Writing `dest` in place would fail
/// with ETXTBSY whenever it is the currently running binary (`malboxctl
/// install` re-installing itself, every upgrade); rename swaps the directory
/// entry while the running process keeps its old inode.
pub(crate) fn write_executable(
    reader: &mut dyn Read,
    dest: &Path,
    step: Step,
) -> crate::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let parent = dest.parent().ok_or_else(|| InstallError::StepFailed {
        step,
        message: format!("{} has no parent directory", dest.display()),
    })?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent)
        .step_ctx(step, "failed to create temporary file")?;
    std::io::copy(reader, tmp.as_file_mut())
        .step_ctx(step, &format!("failed to write {}", dest.display()))?;
    tmp.as_file()
        .set_permissions(std::fs::Permissions::from_mode(0o755))
        .step_ctx(step, "failed to set permissions")?;
    tmp.persist(dest)
        .step_ctx(step, &format!("failed to install {}", dest.display()))?;
    Ok(())
}

pub(crate) fn extract_binary(
    bytes: &[u8],
    binary_name: &str,
    dest: &Path,
    step: Step,
) -> crate::Result<()> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive
        .entries()
        .step_ctx(step, "failed to read tarball entries")?
    {
        let mut entry = entry.step_ctx(step, "failed to read tarball entry")?;
        let path = entry.path().step_ctx(step, "failed to read entry path")?;

        if path.file_name().is_some_and(|n| n == binary_name) {
            return write_executable(&mut entry, dest, step);
        }
    }

    Err(InstallError::StepFailed {
        step,
        message: format!("binary '{binary_name}' not found in tarball"),
    })
}

/// GitHub source tarballs wrap everything in a single `repo-version/`
/// directory; return it when it is the only entry, otherwise the base.
pub(crate) fn find_extracted_dir(base: &Path, step: Step) -> crate::Result<PathBuf> {
    let entries = std::fs::read_dir(base).step_ctx(step, "failed to read extracted directory")?;

    let mut dirs = Vec::new();
    let mut file_count = 0usize;
    for entry in entries {
        let entry = entry.step_ctx(step, "failed to read directory entry")?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else {
            file_count += 1;
        }
    }

    match (dirs.len(), file_count) {
        (0, 0) => Err(InstallError::StepFailed {
            step,
            message: "empty archive".to_string(),
        }),
        (1, 0) => Ok(dirs.remove(0)),
        _ => Ok(base.to_path_buf()),
    }
}
