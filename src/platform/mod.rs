//! Everything that differs between operating systems.
//!
//! BleachBit keeps three big modules (`Unix.py`, `Windows.py`,
//! `WindowsWipe.py`) and imports the right one dynamically. Rust resolves the
//! same problem at compile time with `#[cfg]`, which means a Linux binary
//! carries no Windows code at all — one of the reasons the resulting binary is
//! a couple of megabytes instead of a Python runtime plus 21 000 lines of
//! interpreted source.

use std::path::{Path, PathBuf};

use crate::core::error::Result;
// The macOS/BSD branch of `trim()` reports through `Error::msg`; importing it
// only where it is actually used keeps Linux and Windows warning-free (those
// targets fall back to the `not(any(unix, windows))` arms instead).
#[cfg(all(unix, not(target_os = "linux")))]
use crate::core::error::Error;

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

/// The operating system Sweep was built for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Linux,
    Macos,
    Bsd,
    /// Any other POSIX system (Solaris, illumos, ...).
    UnixOther,
}

/// Which OS are we running on?
pub fn current_os() -> Os {
    #[cfg(windows)]
    {
        Os::Windows
    }
    #[cfg(target_os = "linux")]
    {
        Os::Linux
    }
    #[cfg(target_os = "macos")]
    {
        Os::Macos
    }
    #[cfg(any(
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    {
        Os::Bsd
    }
    #[cfg(not(any(
        windows,
        target_os = "linux",
        target_os = "macos",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )))]
    {
        Os::UnixOther
    }
}

/// Does the `os="..."` attribute of a cleaner definition match this machine?
///
/// Semantics are identical to BleachBit's `General.os_match()`:
/// `""` matches everything; `unix` matches every POSIX system; `bsd` matches
/// every BSD; `darwin` is accepted as a deprecated alias for `macos`.
pub fn os_matches(filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let filter = if filter == "darwin" { "macos" } else { filter };

    let current = current_os();
    match filter {
        "windows" => current == Os::Windows,
        "linux" => current == Os::Linux,
        "macos" => current == Os::Macos,
        "unix" => matches!(current, Os::Linux | Os::Macos | Os::Bsd | Os::UnixOther),
        "bsd" => matches!(
            current,
            Os::Bsd | Os::Macos // macOS is certified UNIX and behaves like a BSD here
        ),
        "freebsd" | "openbsd" | "netbsd" | "dragonfly" => {
            current == Os::Bsd && specific_bsd_matches(filter)
        }
        other => {
            log::debug!("unknown os filter '{other}' — treating as no match");
            false
        }
    }
}

#[cfg(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn specific_bsd_matches(filter: &str) -> bool {
    filter == std::env::consts::OS
}

#[cfg(not(any(
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
fn specific_bsd_matches(_filter: &str) -> bool {
    false
}

/// Short OS name used in log output and in the `--json` report.
pub fn os_name() -> &'static str {
    match current_os() {
        Os::Windows => "windows",
        Os::Linux => "linux",
        Os::Macos => "macos",
        Os::Bsd => "bsd",
        Os::UnixOther => "unix",
    }
}

/// Current user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// User-level cache directory (`~/.cache`, `~/Library/Caches`, `%LOCALAPPDATA%`).
pub fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir()
}

/// Per-user configuration directory.
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("sweep"))
}

/// System-wide cleaner definition directories (read-only, trusted).
pub fn system_cleaner_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(unix)]
    {
        dirs.push(PathBuf::from("/usr/share/sweep/cleaners"));
        dirs.push(PathBuf::from("/usr/local/share/sweep/cleaners"));
        dirs.push(PathBuf::from("/etc/sweep/cleaners"));
    }
    #[cfg(windows)]
    {
        if let Ok(dir) = std::env::var("ProgramFiles") {
            dirs.push(PathBuf::from(dir).join("sweep").join("cleaners"));
        }
    }
    dirs
}

/// Directories searched for BleachBit's CleanerML files when
/// `--import-bleachbit` is used without an explicit path.
pub fn bleachbit_cleaner_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    #[cfg(unix)]
    {
        dirs.push(PathBuf::from("/usr/share/bleachbit/cleaners"));
        dirs.push(PathBuf::from("/usr/local/share/bleachbit/cleaners"));
    }
    #[cfg(windows)]
    {
        if let Ok(dir) = std::env::var("ProgramFiles") {
            dirs.push(PathBuf::from(dir).join("BleachBit").join("cleaners"));
        }
        if let Ok(dir) = std::env::var("ProgramFiles(x86)") {
            dirs.push(PathBuf::from(dir).join("BleachBit").join("cleaners"));
        }
    }
    dirs
}

// ---------------------------------------------------------------------------
// Filesystem primitives with per-platform implementations
// ---------------------------------------------------------------------------

/// Filesystem identifier for `path` (POSIX `st_dev`).
pub fn device_id(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(path).ok().map(|md| md.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        None
    }
}

/// Is `path` a hard link with more than one name?
pub fn is_hard_link(path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        std::fs::symlink_metadata(path)
            .map(|md| md.file_type().is_file() && md.nlink() > 1)
            .unwrap_or(false)
    }
    #[cfg(windows)]
    {
        windows::is_hard_link(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        false
    }
}

/// Windows reparse point (symlink or junction). Always `false` on POSIX.
pub fn is_reparse_point(path: &Path) -> bool {
    #[cfg(windows)]
    {
        windows::is_reparse_point(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// Is `path` a mount point?
pub fn is_mount_point(path: &Path) -> bool {
    #[cfg(unix)]
    {
        unix::is_mount_point(path)
    }
    #[cfg(windows)]
    {
        windows::is_mount_point(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        false
    }
}

/// Does this I/O error mean "the file is locked by another process"?
pub fn is_locked(err: &std::io::Error) -> bool {
    #[cfg(windows)]
    {
        matches!(err.raw_os_error(), Some(32) | Some(33))
    }
    #[cfg(unix)]
    {
        matches!(err.raw_os_error(), Some(16) | Some(11))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = err;
        false
    }
}

/// Does this I/O error mean "read-only / access denied"?
pub fn is_readonly_error(err: &std::io::Error) -> bool {
    err.kind() == std::io::ErrorKind::PermissionDenied
}

/// Drop the read-only attribute if the platform has one.
pub fn clear_readonly(path: &Path) -> bool {
    #[cfg(windows)]
    {
        windows::clear_readonly(path)
    }
    #[cfg(unix)]
    {
        unix::make_writable(path).is_ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        false
    }
}

/// Free space at `path`.
pub fn free_space(path: &Path) -> Result<u64> {
    #[cfg(unix)]
    {
        unix::free_space(path)
    }
    #[cfg(windows)]
    {
        windows::free_space(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(Error::msg(
            "free space reporting is unsupported on this platform",
        ))
    }
}

/// Total capacity of the filesystem holding `path`.
pub fn total_space(path: &Path) -> Result<u64> {
    #[cfg(unix)]
    {
        unix::total_space(path)
    }
    #[cfg(windows)]
    {
        windows::total_space(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(Error::msg(
            "capacity reporting is unsupported on this platform",
        ))
    }
}

/// Issue a discard/TRIM for the filesystem holding `path`.
pub fn trim(path: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        linux::fitrim(path)
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let _ = path;
        Err(Error::msg("trim is only implemented on Linux"))
    }
    #[cfg(windows)]
    {
        windows::trim(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(Error::msg("trim is unsupported on this platform"))
    }
}

/// Flush all filesystem buffers (`sync(2)` / `_flushall()`).
pub fn sync_filesystems() {
    #[cfg(unix)]
    {
        unix::sync_filesystems();
    }
    #[cfg(windows)]
    {
        windows::flush_all();
    }
}

/// One-time process setup that has to happen before anything is printed.
///
/// On Windows the console defaults to the legacy ANSI code page, which mangles
/// the non-ASCII cleaner labels and file paths Sweep reports. Elsewhere there
/// is nothing to do, so the call site stays unconditional.
pub fn prepare_process() {
    #[cfg(windows)]
    windows::init_console();
}

/// Is a process with this executable name running?
///
/// A full process-table refresh costs milliseconds; the worker asks once per
/// cleaner option (dozens of times per run), so results are cached for a few
/// seconds. Stale-by-seconds is fine here — the check is a safety heuristic,
/// not a lock.
pub fn is_process_running(name: &str) -> bool {
    use std::collections::HashSet;
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    struct Snapshot {
        at: Instant,
        names: HashSet<String>,
    }

    static CACHE: std::sync::OnceLock<Mutex<Option<Snapshot>>> = std::sync::OnceLock::new();
    const TTL: Duration = Duration::from_secs(5);

    let needle = name.to_lowercase();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(guard) = cache.lock() {
        if let Some(snapshot) = guard.as_ref() {
            if snapshot.at.elapsed() < TTL {
                return snapshot.names.contains(&needle);
            }
        }
    }

    let system = sysinfo::System::new_all();
    let names: HashSet<String> = system
        .processes()
        .values()
        .map(|process| process.name().to_string().to_lowercase())
        .collect();
    let hit = names.contains(&needle);
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(Snapshot {
            at: Instant::now(),
            names,
        });
    }
    hit
}

/// Clear the system clipboard. Only meaningful with a GUI session attached.
pub fn clear_clipboard() -> Result<()> {
    #[cfg(windows)]
    {
        windows::clear_clipboard()
    }
    #[cfg(unix)]
    {
        unix::clear_clipboard()
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::msg("clipboard clearing is unsupported"))
    }
}

/// Flush the system DNS resolver cache.
pub fn flush_dns() -> Result<()> {
    #[cfg(windows)]
    {
        windows::flush_dns()
    }
    #[cfg(target_os = "linux")]
    {
        linux::flush_dns()
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        unix::flush_dns()
    }
    #[cfg(not(any(unix, windows)))]
    {
        Err(Error::msg("dns flushing is unsupported"))
    }
}

/// Directories holding the desktop trash / recycle bin contents.
pub fn trash_dirs() -> Vec<PathBuf> {
    #[cfg(unix)]
    {
        unix::trash_dirs()
    }
    #[cfg(windows)]
    {
        windows::recycle_bin_dirs()
    }
    #[cfg(not(any(unix, windows)))]
    {
        Vec::new()
    }
}

/// Rotated/compressed system logs (`/var/log/*.gz`, `*.1`, ...).
pub fn rotated_logs() -> Vec<PathBuf> {
    #[cfg(unix)]
    {
        unix::rotated_logs()
    }
    #[cfg(windows)]
    {
        Vec::new()
    }
    #[cfg(not(any(unix, windows)))]
    {
        Vec::new()
    }
}

/// Localisation directories that can be pruned, keeping `keep`.
pub fn localization_paths(keep: &[String]) -> Vec<PathBuf> {
    #[cfg(unix)]
    {
        unix::localization_paths(keep)
    }
    #[cfg(not(unix))]
    {
        let _ = keep;
        Vec::new()
    }
}

/// Notify the desktop shell that files changed (Windows `SHChangeNotify`).
pub fn shell_refresh() {
    #[cfg(windows)]
    {
        windows::shell_change_notify();
    }
    #[cfg(not(windows))]
    {
        // GTK/X11 pick up directory changes automatically.
    }
}

/// Any platform-specific startup diagnostics for `sweep doctor`.
pub fn diagnostics() -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!("os: {}", os_name()));
    if let Some(home) = home_dir() {
        out.push(format!("home: {}", home.display()));
    }
    if let Some(cache) = cache_dir() {
        out.push(format!("cache: {}", cache.display()));
    }
    out.push(format!("cpus: {}", num_cpus::get()));

    #[cfg(unix)]
    out.extend(unix::diagnostics());
    #[cfg(windows)]
    out.extend(windows::diagnostics());

    out
}
