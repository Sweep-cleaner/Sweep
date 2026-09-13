//! Free-space wiping and SSD trim.
//!
//! BleachBit's `wipe_path()` creates one enormous temporary file and shrinks
//! its name on ENOSPC. Sweep writes a series of fixed-size chunks
//! instead, which:
//!
//! * survives filesystems with small maximum file sizes (FAT32, ext3 with
//!   2 GiB limits);
//! * reports progress continuously;
//! * keeps peak memory flat at `chunk_size` bytes;
//! * registers every generated file in a guard so an abort or panic cannot
//!   leave gigabytes of junk behind.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rand::distributions::{Alphanumeric, DistString};

use crate::core::error::{Error, Result};
use crate::fsutil::delete::DeleteOptions;
use crate::fsutil::size::free_space;
use crate::shred::Pattern;

/// Prefix used for the temporary fill files, so leftovers can always be found.
pub const FILL_PREFIX: &str = "sweep_fill_";

/// Default chunk size: 64 MiB.
pub const DEFAULT_CHUNK: u64 = 64 * 1024 * 1024;

/// Progress callback for the long-running free-space wipe.
pub trait WipeProgress: Send + Sync {
    /// Called with (bytes written so far, bytes still to write).
    fn report(&self, written: u64, remaining: u64);
}

/// Progress sink that does nothing.
pub struct NullWipeProgress;

impl WipeProgress for NullWipeProgress {
    fn report(&self, _written: u64, _remaining: u64) {}
}

/// Configuration for a free-space wipe.
#[derive(Debug, Clone, Copy)]
pub struct WipeConfig {
    /// Size of one fill file.
    pub chunk_size: u64,
    /// Overwrite material.
    pub pattern: Pattern,
    /// Stop early instead of filling the partition to the very last byte.
    ///
    /// Leaving a small reserve avoids starving the rest of the system while
    /// the wipe runs.
    pub reserve: u64,
}

impl Default for WipeConfig {
    fn default() -> Self {
        Self {
            chunk_size: DEFAULT_CHUNK,
            pattern: Pattern::Zeros,
            reserve: 1024 * 1024,
        }
    }
}

/// Guard that removes every fill file it created, even after a panic.
struct FillGuard {
    files: Vec<PathBuf>,
    active: bool,
}

impl FillGuard {
    fn new() -> Self {
        Self {
            files: Vec::new(),
            active: true,
        }
    }

    fn push(&mut self, path: PathBuf) {
        self.files.push(path);
    }

    /// Detach the last file: it is about to be removed by the caller itself.
    fn pop(&mut self) -> Option<PathBuf> {
        self.files.pop()
    }

    fn disarm(&mut self) {
        self.active = false;
        self.files.clear();
    }
}

impl Drop for FillGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        for file in self.files.drain(..) {
            if let Err(err) = std::fs::remove_file(&file) {
                log::warn!("could not remove fill file {}: {}", file.display(), err);
            }
        }
    }
}

/// Overwrite the free space of the filesystem containing `dir`.
///
/// Returns the number of bytes written.
pub fn wipe_free_space(
    dir: &Path,
    config: WipeConfig,
    progress: &dyn WipeProgress,
    cancel: &AtomicBool,
) -> Result<u64> {
    if !dir.is_dir() {
        return Err(Error::msg(format!("not a directory: {}", dir.display())));
    }

    // Clean up anything a previous crashed run left behind.
    let orphans = detect_orphaned_fill_files(dir);
    if !orphans.is_empty() {
        log::info!(
            "removing {} orphaned fill file(s) in {}",
            orphans.len(),
            dir.display()
        );
        for orphan in orphans {
            // Route through the audited delete path rather than a raw
            // `remove_file`, and tolerate a file that vanished in between.
            if let Err(err) = crate::fsutil::delete::delete(&orphan, DeleteOptions::lenient()) {
                log::debug!(
                    "could not remove orphaned fill file {}: {}",
                    orphan.display(),
                    err
                );
            }
        }
    }

    let mut target = free_space(dir)?;
    if target <= config.reserve {
        return Ok(0);
    }
    target -= config.reserve;

    let mut guard = FillGuard::new();
    let mut written: u64 = 0;
    let mut buffer = config.pattern.filler(0, 1024 * 1024);

    while written < target {
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        let remaining = target - written;
        let this_chunk = remaining.min(config.chunk_size);
        let path = unique_fill_path(dir);

        let result = write_fill_file(&path, this_chunk, &mut buffer, config, cancel);

        match result {
            Ok(bytes) => {
                if bytes == 0 {
                    // Disk is full for practical purposes.
                    let _ = std::fs::remove_file(&path);
                    break;
                }
                written += bytes;
                guard.push(path.clone());
                progress.report(written, target.saturating_sub(written));

                // Remove immediately: the point is to overwrite the blocks,
                // not to keep a big file around. Deleting as we go also keeps
                // the directory small.
                guard.pop();
                if let Err(err) = std::fs::remove_file(&path) {
                    log::warn!("could not remove fill file {}: {}", path.display(), err);
                }
            }
            Err(err) => {
                log::debug!("fill file {} failed: {}", path.display(), err);
                let _ = std::fs::remove_file(&path);
                // ENOSPC means we are done; anything else is worth reporting.
                if is_no_space(&err) {
                    break;
                }
                if !err.is_benign() {
                    return Err(err);
                }
                break;
            }
        }
    }

    guard.disarm();
    Ok(written)
}

/// Write one fill file of at most `size` bytes.
fn write_fill_file(
    path: &Path,
    size: u64,
    buffer: &mut [u8],
    config: WipeConfig,
    cancel: &AtomicBool,
) -> Result<u64> {
    use std::io::Write;

    let mut file = std::fs::File::create(path).map_err(|e| Error::io(path, e))?;
    let mut remaining = size;

    while remaining > 0 {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let take = remaining.min(buffer.len() as u64) as usize;
        if config.pattern == Pattern::Random {
            regenerate(buffer);
        }
        match file.write_all(&buffer[..take]) {
            Ok(()) => remaining -= take as u64,
            Err(err) => {
                // Sync what we have, then let the caller interpret the error.
                let _ = file.sync_data();
                let written = size - remaining;
                return if written > 0 {
                    Ok(written)
                } else {
                    Err(Error::io(path, err))
                };
            }
        }
    }

    file.sync_all().map_err(|e| Error::io(path, e))?;
    Ok(size - remaining)
}

fn regenerate(buffer: &mut [u8]) {
    use rand::Rng;
    rand::thread_rng().fill(buffer);
}

/// True when `err` signals a full disk. OS error messages follow the system
/// locale, so the errno is checked first and the English text is only a
/// fallback (ENOSPC = 28 on Linux/macOS, ERROR_DISK_FULL = 112 on Windows).
fn is_no_space(err: &Error) -> bool {
    let from_errno = match err {
        Error::Io { source, .. } | Error::PlainIo(source) => {
            matches!(source.raw_os_error(), Some(28) | Some(112))
        }
        _ => false,
    };
    from_errno
        || err.to_string().contains("No space left")
        || err.to_string().to_lowercase().contains("not enough space")
}

/// Unique, recognisable name for a fill file.
fn unique_fill_path(dir: &Path) -> PathBuf {
    let suffix = Alphanumeric.sample_string(&mut rand::thread_rng(), 24);
    dir.join(format!("{FILL_PREFIX}{suffix}"))
}

/// Find leftovers from an interrupted run (BleachBit's
/// `detect_orphaned_wipe_files()`).
pub fn detect_orphaned_fill_files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return found,
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(FILL_PREFIX) {
            found.push(entry.path());
        }
    }
    found
}

/// Ask the filesystem to discard unused blocks (`fstrim` on Linux,
/// `FSCTL_FILE_LEVEL_TRIM` where available).
///
/// On SSDs this is strictly better than overwriting: it is instant and does
/// not consume write cycles.
pub fn trim(path: &Path) -> Result<()> {
    crate::platform::trim(path)
}
