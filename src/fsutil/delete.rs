//! Deletion, truncation and the "file in use" fallbacks.
//!
//! Ported from BleachBit's `FileUtilities.delete()` with one behavioural
//! improvement: every failure is returned as a typed [`Error`] instead of
//! raising, so the worker can decide per case whether to abort or continue.

use std::fs;
use std::path::{Path, PathBuf};

use crate::core::error::{Error, Result};
use crate::shred::{shred_file, truncate_contents, ShredSpec};

/// Everything that controls a single delete.
#[derive(Debug, Clone, Copy)]
pub struct DeleteOptions {
    /// Overwrite before unlinking.
    pub shred: bool,
    /// A missing path is not an error.
    pub ignore_missing: bool,
    /// Honour the `shred` flag at all (some paths must never be shredded,
    /// e.g. the temporary fill files used by the free-space wiper).
    pub allow_shred: bool,
    /// Overwrite material and pass count.
    pub spec: ShredSpec,
}

impl Default for DeleteOptions {
    fn default() -> Self {
        Self {
            shred: false,
            ignore_missing: false,
            allow_shred: true,
            spec: ShredSpec::default_spec(),
        }
    }
}

impl DeleteOptions {
    /// Plain unlink.
    pub fn simple() -> Self {
        Self::default()
    }

    /// Unlink, tolerating a path that is already gone.
    ///
    /// Used for the temporary fill files a crashed wipe run can leave behind:
    /// they may disappear between being listed and being removed.
    pub fn lenient() -> Self {
        Self {
            ignore_missing: true,
            ..Self::default()
        }
    }

    /// Shredded delete.
    pub fn shred(spec: ShredSpec) -> Self {
        Self {
            shred: true,
            spec,
            ..Self::default()
        }
    }
}

/// Is `path` a hard link with more than one name?
pub fn is_hard_link(path: &Path) -> bool {
    crate::platform::is_hard_link(path)
}

/// Is `path` a directory that still has entries?
fn dir_is_empty(path: &Path) -> bool {
    match fs::read_dir(path) {
        Ok(mut it) => it.next().is_none(),
        Err(_) => false,
    }
}

/// Is `path` a mount point? Deleting one is never what the user wanted.
fn is_mount_point(path: &Path) -> bool {
    crate::platform::is_mount_point(path)
}

/// Delete a file, directory, symlink, junction or FIFO.
///
/// Returns `true` when something was actually removed.
///
/// Safety: this performs NO keep-list, protected-path, or link-target checks.
/// Every call site must pass through [`crate::core::keep::Guard::check`] (or
/// [`crate::deep::guarded_delete`]) first — deleting without the guard is a
/// security bug, not an optimization.
pub fn delete(path: &Path, options: DeleteOptions) -> Result<bool> {
    let path = &crate::core::path::extended_path(path);

    let md = match fs::symlink_metadata(path) {
        Ok(md) => md,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            if options.ignore_missing {
                return Ok(false);
            }
            return Err(Error::io(path, err));
        }
        Err(err) => return Err(Error::io(path, err)),
    };

    // ---- links, FIFOs, sockets: remove the name, never the target ---------
    if md.file_type().is_symlink() {
        return remove_any(path, options);
    }
    if !md.file_type().is_file() && !md.file_type().is_dir() {
        // FIFO / socket / device node
        return remove_any(path, options);
    }
    #[cfg(windows)]
    if crate::platform::is_reparse_point(path) {
        return remove_any(path, options);
    }

    // ---- directories -------------------------------------------------------
    if md.file_type().is_dir() {
        return delete_dir(path, options);
    }

    // ---- regular files -----------------------------------------------------
    if options.shred && options.allow_shred {
        shred_file(path, options.spec)?;
        return Ok(true);
    }

    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) => {
            if crate::platform::is_locked(&err) {
                // Best-effort truncate so the contents do not survive, then
                // report the failure so the caller can schedule a reboot
                // delete (Windows) or just log it.
                let _ = truncate_contents(path);
                Err(Error::io(path, err))
            } else if crate::platform::is_readonly_error(&err)
                && crate::platform::clear_readonly(path)
            {
                fs::remove_file(path)
                    .map(|()| true)
                    .map_err(|e| Error::io(path, e))
            } else {
                Err(Error::io(path, err))
            }
        }
    }
}

fn remove_any(path: &Path, _options: DeleteOptions) -> Result<bool> {
    fs::remove_file(path)
        .map(|()| true)
        .or_else(|err| {
            if crate::platform::is_readonly_error(&err) && crate::platform::clear_readonly(path) {
                fs::remove_file(path).map(|()| true)
            } else {
                Err(err)
            }
        })
        .map_err(|e| Error::io(path, e))
}

fn delete_dir(path: &Path, options: DeleteOptions) -> Result<bool> {
    let mut target: PathBuf = path.to_path_buf();

    if options.shred && options.allow_shred {
        if !dir_is_empty(path) {
            log::info!("directory is not empty, skipping shred: {}", path.display());
            return Ok(false);
        }
        // Shredding a directory only means hiding its name; there is no
        // content to overwrite.
        target = crate::shred::rename_random(path)?;
    }

    match fs::remove_dir(&target) {
        Ok(()) => Ok(true),
        Err(err) => {
            let raw = err.raw_os_error();
            if err.kind() == std::io::ErrorKind::Other
                || err.to_string().contains("not empty")
                || raw == Some(145) // ERROR_DIR_NOT_EMPTY
                || raw == Some(39)
            // ENOTEMPTY
            {
                log::info!("directory is not empty: {}", path.display());
                Ok(false)
            } else if raw == Some(16) || raw == Some(32) {
                // EBUSY / ERROR_SHARING_VIOLATION
                if is_mount_point(path) {
                    log::info!("skipping mount point: {}", path.display());
                } else {
                    log::info!("device or resource busy: {}", path.display());
                }
                Ok(false)
            } else if crate::platform::is_readonly_error(&err)
                && crate::platform::clear_readonly(&target)
            {
                fs::remove_dir(&target)
                    .map(|()| true)
                    .map_err(|e| Error::io(path, e))
            } else {
                Err(Error::io(path, err))
            }
        }
    }
}

/// Truncate a file to zero bytes, refusing to follow a link.
pub fn truncate_file(path: &Path) -> Result<()> {
    truncate_contents(path)
}

/// Delete a list of paths, returning how many actually disappeared.
///
/// Runs in parallel when the list is large — safe because the paths come from
/// a `contents_first` scan, i.e. children always precede their parent.
///
/// Safety: like [`delete`], this performs NO guard checks. Only call it with
/// paths that already passed [`crate::core::keep::Guard::check`].
pub fn delete_all(paths: &[PathBuf], options: DeleteOptions) -> usize {
    use rayon::prelude::*;

    if paths.len() < 64 {
        paths
            .iter()
            .filter(|p| delete(p, options).unwrap_or(false))
            .count()
    } else {
        paths
            .par_iter()
            .filter(|p| delete(p, options).unwrap_or(false))
            .count()
    }
}
