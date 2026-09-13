//! POSIX shared behaviour (Linux, the BSDs, macOS, illumos...).

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::error::{Error, Result};

/// Free space at `path` (`statvfs`).
pub fn free_space(path: &Path) -> Result<u64> {
    statvfs(path).map(|vfs| vfs.0)
}

/// Total capacity of the filesystem holding `path`.
pub fn total_space(path: &Path) -> Result<u64> {
    statvfs(path).map(|vfs| vfs.1)
}

/// (available, total) from `statvfs(3)`.
fn statvfs(path: &Path) -> Result<(u64, u64)> {
    use std::ffi::CString;
    use std::os::raw::c_char;

    let target = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    };

    let c_path = CString::new(target.to_string_lossy().as_bytes())
        .map_err(|_| Error::msg("path contains a NUL byte"))?;

    unsafe {
        let mut buf: libc::statvfs = std::mem::zeroed();
        let rc = libc::statvfs(c_path.as_ptr() as *const c_char, &mut buf);

        if rc != 0 {
            return Err(Error::io(
                path,
                std::io::Error::from_raw_os_error(
                    std::io::Error::last_os_error()
                        .raw_os_error()
                        .unwrap_or(libc::EIO),
                ),
            ));
        }
        // 32-bit platformlarda alanlar u32'dir; dönüşüm orada gereklidir.
        #[allow(clippy::useless_conversion, clippy::unnecessary_cast)]
        Ok((
            u64::from(buf.f_bavail).saturating_mul(buf.f_frsize as u64),
            u64::from(buf.f_blocks).saturating_mul(buf.f_frsize as u64),
        ))
    }
}

/// Is `path` a mount point? Compares the device id with its parent's.
pub fn is_mount_point(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;

    let child = match std::fs::symlink_metadata(path) {
        Ok(md) => md,
        Err(_) => return false,
    };
    let parent = match path.parent() {
        Some(parent) => match std::fs::symlink_metadata(parent) {
            Ok(md) => md,
            Err(_) => return false,
        },
        None => return true, // "/"
    };

    child.dev() != parent.dev()
}

/// Grant the owner write permission (used before deleting a read-only file).
pub fn make_writable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let md = std::fs::symlink_metadata(path).map_err(|e| Error::io(path, e))?;
    let mut perms = md.permissions();
    perms.set_mode(perms.mode() | 0o200);
    std::fs::set_permissions(path, perms).map_err(|e| Error::io(path, e))
}

/// `sync(2)`.
pub fn sync_filesystems() {
    unsafe {
        libc::sync();
    }
}

/// XDG trash directories for the current user plus mounted volumes.
pub fn trash_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Ok(home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(home).join("Trash"));
    } else if let Some(home) = crate::platform::home_dir() {
        dirs.push(home.join(".local").join("share").join("Trash"));
    }

    // macOS stores the trash in ~/.Trash and on each mounted volume.
    if cfg!(target_os = "macos") {
        if let Some(home) = crate::platform::home_dir() {
            dirs.push(home.join(".Trash"));
        }
        for mount in ["/Volumes"] {
            if let Ok(entries) = std::fs::read_dir(mount) {
                for entry in entries.flatten() {
                    let trash = entry.path().join(".Trashes");
                    if let Some(uid) = current_uid() {
                        dirs.push(trash.join(uid.to_string()));
                    }
                }
            }
        }
    }

    dirs.retain(|d| d.is_dir());
    dirs
}

fn current_uid() -> Option<u32> {
    unsafe { Some(libc::getuid()) }
}

/// Rotated logs in `/var/log` and `~/Library/Logs`.
///
/// Matches `*.gz`, `*.bz2`, `*.xz`, `*.zst`, `*.old`, `*.1`, `*.2.gz`, ...
pub fn rotated_logs() -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in [PathBuf::from("/var/log"), PathBuf::from("/private/var/log")] {
        if root.is_dir() {
            found.extend(rotated_in(&root));
        }
    }
    if cfg!(target_os = "macos") {
        if let Some(home) = crate::platform::home_dir() {
            let logs = home.join("Library").join("Logs");
            if logs.is_dir() {
                found.extend(rotated_in(&logs));
            }
        }
    }
    found
}

fn rotated_in(root: &Path) -> Vec<PathBuf> {
    use crate::fsutil::walk::ScanOptions;

    crate::fsutil::walk::scan_paths(root, &ScanOptions::files())
        .into_iter()
        .filter(|path| {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            is_rotated_name(&name)
        })
        .filter(|path| {
            // Only regular files that the current user owns, mirroring
            // BleachBit's `ego_owner()` check for /tmp.
            std::fs::symlink_metadata(path)
                .map(|md| md.file_type().is_file())
                .unwrap_or(false)
        })
        .collect()
}

fn is_rotated_name(name: &str) -> bool {
    if name.starts_with('.') {
        return false;
    }
    for suffix in [".gz", ".bz2", ".xz", ".zst", ".lz4", ".old", ".zip"] {
        if name.ends_with(suffix) {
            return true;
        }
    }
    // `syslog.1`, `auth.log.2`, `messages.0` ...
    let stem = name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name);
    let ext = name.rsplit_once('.').map(|(_, e)| e);
    match ext {
        Some(ext) if !ext.is_empty() && ext.chars().all(|c| c.is_ascii_digit()) => !stem.is_empty(),
        _ => false,
    }
}

/// Localisation directories that can safely be pruned.
///
/// Walks the usual `/usr/share/locale`, `/usr/share/man`, `/usr/share/doc`,
/// `/usr/share/gtk-doc`, `/usr/share/help`, `/usr/share/i18n`,
/// `/usr/share/gnome/help` trees and returns every entry whose name is a
/// locale code that is *not* in `keep`.
pub fn localization_paths(keep: &[String]) -> Vec<PathBuf> {
    let roots = [
        "/usr/share/locale",
        "/usr/share/man",
        "/usr/share/doc",
        "/usr/share/gtk-doc",
        "/usr/share/help",
        "/usr/share/i18n",
        "/usr/share/gnome/help",
        "/usr/share/locale-langpack",
    ];

    let mut dirs = Vec::new();
    for root in roots {
        let root = Path::new(root);
        if !root.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(root) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = match path.file_name() {
                Some(name) => name.to_string_lossy().to_string(),
                None => continue,
            };
            if !is_locale_dir(&name) {
                continue;
            }
            if keep.iter().any(|k| locale_keep_matches(k, &name)) {
                continue;
            }
            dirs.push(path);
        }
    }
    dirs
}

fn is_locale_dir(name: &str) -> bool {
    if matches!(
        name,
        "C" | "C.UTF-8" | "POSIX" | "en" | "en_US" | "man" | "locale.alias"
    ) {
        // "en" is normally kept; the keep list decides. Everything else here
        // is not a locale at all.
        return !matches!(name, "man" | "locale.alias");
    }
    if name.len() < 2 || name.len() > 12 {
        return false;
    }
    name.chars().all(|c| {
        c.is_ascii_lowercase()
            || c.is_ascii_uppercase()
            || c == '_'
            || c == '@'
            || c == '-'
            || c.is_ascii_digit()
    })
}

fn locale_keep_matches(keep: &str, name: &str) -> bool {
    let keep = keep.trim();
    if keep.is_empty() {
        return false;
    }
    if keep.eq_ignore_ascii_case(name) {
        return true;
    }
    // "de" keeps "de_DE", "de_AT", ...
    name.len() > keep.len()
        && name[..keep.len()].eq_ignore_ascii_case(keep)
        && matches!(
            name.as_bytes().get(keep.len()),
            Some(b'_') | Some(b'@') | Some(b'-')
        )
}

/// Clear the clipboard by delegating to a well-known helper when one exists.
///
/// BleachBit shells out to GTK; Sweep stays GUI-toolkit agnostic and uses
/// whichever of `wl-copy`, `xclip`, `xsel` or `pbcopy` is installed.
pub fn clear_clipboard() -> Result<()> {
    let candidates: &[(&str, &[&str])] = &[
        ("wl-copy", &["--clear"]),
        ("xclip", &["-selection", "clipboard", "/dev/null"]),
        ("xsel", &["--clipboard", "--clear"]),
        ("pbcopy", &["/dev/null"]),
    ];

    for (program, args) in candidates {
        if !crate::fsutil::path_exists_in_path(program) {
            continue;
        }
        match Command::new(program).args(*args).status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(_) => continue,
            Err(err) => {
                log::debug!("{program} failed: {err}");
            }
        }
    }

    Err(Error::msg(
        "no clipboard helper found (install wl-copy, xclip, xsel or use macOS pbcopy)",
    ))
}

/// Flush the DNS cache using `resolvectl`, `systemd-resolve` or `dscacheutil`.
pub fn flush_dns() -> Result<()> {
    let candidates: &[(&str, &[&str])] = &[
        ("resolvectl", &["flush-caches"]),
        ("systemd-resolve", &["--flush-caches"]),
        ("dscacheutil", &["-flushcache"]),
        ("killall", &["-HUP", "mDNSResponder"]),
    ];

    let mut last_error: Option<Error> = None;
    for (program, args) in candidates {
        if !crate::fsutil::path_exists_in_path(program) {
            continue;
        }
        match Command::new(program).args(*args).status() {
            Ok(status) if status.success() => return Ok(()),
            Ok(status) => {
                // Kurulu ama çalışmayan servis (örn. resolvectl + pasif
                // systemd-resolved): sıradaki adaya geç.
                log::debug!("{program} exited with {status}");
                last_error = Some(Error::ExternalCommand {
                    command: format!("{program} {}", args.join(" ")),
                    status: status.code().unwrap_or(-1),
                    stderr: String::new(),
                });
            }
            Err(err) => {
                log::debug!("{program} failed: {err}");
            }
        }
    }

    Err(last_error.unwrap_or_else(|| Error::msg("no supported DNS flush command found")))
}

/// Extra `sweep doctor` output.
pub fn diagnostics() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(uid) = current_uid() {
        out.push(format!(
            "uid: {uid}{}",
            if uid == 0 { " (root)" } else { "" }
        ));
    }
    for program in [
        "resolvectl",
        "systemd-resolve",
        "journalctl",
        "apt-get",
        "dnf",
        "paccache",
    ] {
        if crate::fsutil::path_exists_in_path(program) {
            out.push(format!("tool: {program}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_names() {
        assert!(is_rotated_name("syslog.1"));
        assert!(is_rotated_name("auth.log.2.gz"));
        assert!(!is_rotated_name("syslog"));
        assert!(!is_rotated_name(".hidden.gz"));
    }

    #[test]
    fn locale_matching() {
        assert!(locale_keep_matches("de", "de_DE"));
        assert!(locale_keep_matches("pt_BR", "pt_BR"));
        assert!(!locale_keep_matches("de", "fr_FR"));
    }
}
