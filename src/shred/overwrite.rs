//! Secure deletion primitives.
//!
//! BleachBit implements `wipe_contents()` (overwrite then truncate),
//! `wipe_name()` (rename to a random same-length name) and, on Windows with
//! admin rights, an FSCTL-based overwrite that handles compressed/sparse
//! files. Sweep ships the portable version of all three plus a
//! configurable multi-pass mode, and drops the admin-only branch in favour of
//! a uniform code path that behaves identically everywhere.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rand::distributions::{Alphanumeric, DistString};
use rand::Rng;

use crate::core::error::{Error, Result};

/// Overwrite material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pattern {
    /// Single pass of zeros. NIST SP 800-88 says this is sufficient for
    /// modern media — and it is the default for a reason: extra passes only
    /// wear out SSDs.
    #[default]
    Zeros,
    /// Single pass of 0xFF.
    Ones,
    /// Single pass of cryptographically-ish random bytes (default RNG).
    Random,
    /// Three passes: zeros, ones, random (DoD 5220.22-M ECE).
    Dod5220,
    /// Seven passes — Gutmann's full sequence is 35 and meaningless on modern
    /// drives, so we offer a sane 7-pass variant for the paranoid.
    Paranoid,
}

impl Pattern {
    /// Number of overwrite passes.
    pub fn passes(self) -> usize {
        match self {
            Pattern::Zeros | Pattern::Ones | Pattern::Random => 1,
            Pattern::Dod5220 => 3,
            Pattern::Paranoid => 7,
        }
    }

    /// The byte filler used for pass `index`.
    pub fn filler(self, index: usize, len: usize) -> Vec<u8> {
        match self {
            Pattern::Zeros => vec![0u8; len],
            Pattern::Ones => vec![0xFFu8; len],
            Pattern::Random => random_bytes(len),
            Pattern::Dod5220 => match index {
                0 => vec![0u8; len],
                1 => vec![0xFFu8; len],
                _ => random_bytes(len),
            },
            Pattern::Paranoid => match index % 3 {
                0 => vec![0u8; len],
                1 => vec![0xFFu8; len],
                _ => random_bytes(len),
            },
        }
    }

    /// Parse a pattern name from config.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_lowercase().as_str() {
            "zeros" | "zero" | "0" => Pattern::Zeros,
            "ones" | "one" | "0xff" => Pattern::Ones,
            "random" | "rand" => Pattern::Random,
            "dod" | "dod5220" | "dod522022mece" => Pattern::Dod5220,
            "paranoid" | "gutmann" => Pattern::Paranoid,
            _ => return None,
        })
    }

    /// Stable string form for the config file.
    pub fn as_str(self) -> &'static str {
        match self {
            Pattern::Zeros => "zeros",
            Pattern::Ones => "ones",
            Pattern::Random => "random",
            Pattern::Dod5220 => "dod",
            Pattern::Paranoid => "paranoid",
        }
    }
}

/// Everything the shredder needs to know.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShredSpec {
    /// Overwrite material and implied pass count.
    pub pattern: Pattern,
    /// Extra passes on top of the pattern's own (0 = just the pattern).
    pub extra_passes: u32,
    /// Rename to a random name before unlinking (hides the name in the
    /// directory entry).
    pub wipe_name: bool,
}

impl ShredSpec {
    /// Default spec: one zero pass plus a name wipe.
    pub fn default_spec() -> Self {
        Self {
            pattern: Pattern::Zeros,
            extra_passes: 0,
            wipe_name: true,
        }
    }

    /// Total number of passes.
    pub fn total_passes(self) -> usize {
        self.pattern.passes() + self.extra_passes as usize
    }
}

fn random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    rand::thread_rng().fill(&mut buf[..]);
    buf
}

/// Open `path` for writing without ever following a final symlink.
///
/// On POSIX this adds `O_NOFOLLOW`, so a link planted after the `is_link()`
/// check still cannot redirect the write. Windows has no `O_NOFOLLOW`; the
/// link check is the only protection available there.
fn open_write_no_follow(path: &Path) -> std::io::Result<File> {
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "refusing to open a link",
        ));
    }

    let mut options = OpenOptions::new();
    options.write(true).create(false).truncate(false);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }

    options.open(path)
}

/// Overwrite a file's contents in place, leaving the length untouched.
///
/// Unlike BleachBit's `wipe_write()` we deliberately do **not** pass
/// `O_TRUNC`: if the write fails half way we would rather keep the (partially
/// destroyed) original than lose the file outright.
pub fn overwrite_contents(path: &Path, spec: ShredSpec) -> Result<()> {
    let len = std::fs::symlink_metadata(path)?.len();
    if len == 0 {
        return Ok(());
    }

    let mut file = match open_write_no_follow(path) {
        Ok(f) => f,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            // Read-only files: temporarily grant ourselves write permission,
            // then restore it. Never done through a link.
            if crate::core::keep::is_link(path) {
                return Err(Error::Link(path.to_path_buf()));
            }
            open_with_write_grant(path)?
        }
        Err(err) => return Err(Error::io(path, err)),
    };

    const CHUNK: usize = 1024 * 1024;

    for pass in 0..spec.total_passes() {
        let chunk = spec.pattern.filler(pass, CHUNK.min(len as usize));
        file.seek(SeekFrom::Start(0))?;
        let mut remaining = len;
        while remaining > 0 {
            let slice = &chunk[..(remaining as usize).min(chunk.len())];
            file.write_all(slice)?;
            remaining -= slice.len() as u64;
        }
        file.flush()?;
        file.sync_data()?;
    }

    Ok(())
}

/// Open a read-only file for writing, restoring its original permissions
/// before returning (the file is shredded, not kept writable).
#[cfg(unix)]
fn open_with_write_grant(path: &Path) -> Result<File> {
    use std::os::unix::fs::PermissionsExt;
    let old = std::fs::metadata(path)
        .map_err(|e| Error::io(path, e))?
        .permissions();
    let mut granted = old.clone();
    granted.set_mode(old.mode() | 0o200);
    std::fs::set_permissions(path, granted).map_err(|e| Error::io(path, e))?;
    let result = open_write_no_follow(path).map_err(|e| Error::io(path, e));
    if std::fs::set_permissions(path, old).is_err() {
        log::warn!("could not restore permissions for {}", path.display());
    }
    result
}

#[cfg(not(unix))]
fn open_with_write_grant(path: &Path) -> Result<File> {
    platform_chmod_write(path)?;
    open_write_no_follow(path).map_err(|e| Error::io(path, e))
}

#[cfg(windows)]
fn platform_chmod_write(path: &Path) -> Result<()> {
    crate::platform::windows::clear_readonly(path);
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn platform_chmod_write(_path: &Path) -> Result<()> {
    Ok(())
}

/// Generate a random filename of exactly `len` characters.
pub fn random_name(len: usize) -> String {
    if len == 0 {
        return String::new();
    }
    Alphanumeric.sample_string(&mut rand::thread_rng(), len)
}

/// Read-only filesystem probe that also compiles on Rust < 1.83
/// (`ErrorKind::ReadOnlyFilesystem` was stabilized in 1.83).
fn is_readonly_fs(err: &std::io::Error) -> bool {
    match err.raw_os_error() {
        #[cfg(unix)]
        Some(code) => code == libc::EROFS,
        #[cfg(windows)]
        Some(code) => code == 19, // ERROR_WRITE_PROTECT
        #[cfg(not(any(unix, windows)))]
        Some(_) => false,
        None => false,
    }
}

/// Rename `path` to a random name of the same length and return the new path.
///
/// Same-length naming matters: on ext3/ext4 a shorter name leaves more of the
/// old directory entry behind, and a longer one may not fit the slack space.
pub fn rename_random(path: &Path) -> Result<PathBuf> {
    let (head, tail) = match (path.parent(), path.file_name()) {
        (Some(head), Some(tail)) => (head, tail.to_string_lossy().to_string()),
        _ => return Ok(path.to_path_buf()),
    };

    for _ in 0..100 {
        let candidate = head.join(random_name(tail.chars().count()));
        if candidate == path || !candidate.exists() {
            return match std::fs::rename(path, &candidate) {
                Ok(()) => Ok(candidate),
                Err(err)
                    if matches!(err.kind(), std::io::ErrorKind::PermissionDenied)
                        || is_readonly_fs(&err) =>
                {
                    // Renaming is a nice-to-have; deletion must still happen.
                    log::debug!("cannot rename {}: {}", path.display(), err);
                    Ok(path.to_path_buf())
                }
                Err(err) => Err(Error::io(path, err)),
            };
        }
    }

    log::debug!(
        "exhausted same-length rename attempts for {}",
        path.display()
    );
    Ok(path.to_path_buf())
}

/// Overwrite, optional rename, then unlink.
pub fn shred_file(path: &Path, spec: ShredSpec) -> Result<()> {
    // Hard links: overwriting would damage the other link's data, and the
    // data survives anyway, so just unlink.
    if crate::platform::is_hard_link(path) {
        std::fs::remove_file(path).map_err(|e| Error::io(path, e))?;
        return Ok(());
    }

    if let Err(err) = overwrite_contents(path, spec) {
        // A locked file cannot be overwritten. BleachBit truncates what it can
        // and marks the rest for deletion on reboot; we log and continue so
        // the rest of the clean run is unaffected.
        log::debug!("overwrite failed for {}: {}", path.display(), err);
        let _ = truncate_contents(path); // best effort
    }

    let target = if spec.wipe_name {
        rename_random(path)?
    } else {
        path.to_path_buf()
    };

    std::fs::remove_file(&target).map_err(|e| Error::io(&target, e))
}

/// Truncate a file to zero bytes without following a link.
pub fn truncate_contents(path: &Path) -> Result<()> {
    if crate::core::keep::is_link(path) {
        return Err(Error::Link(path.to_path_buf()));
    }
    let file = open_write_no_follow(path).map_err(|e| Error::io(path, e))?;
    file.set_len(0)?;
    file.sync_all()?;
    drop(file);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_round_trip() {
        for name in ["zeros", "ones", "random", "dod", "paranoid"] {
            let pattern = Pattern::parse(name).unwrap();
            assert_eq!(pattern.as_str(), name);
        }
        assert!(Pattern::parse("nope").is_none());
    }

    #[test]
    fn human_sizes() {
        assert_eq!(crate::fsutil::size::bytes_to_human(0, false), "0B");
        assert_eq!(crate::fsutil::size::bytes_to_human(999, false), "999B");
        assert_eq!(crate::fsutil::size::bytes_to_human(1500, false), "1.5kB");
    }
}
