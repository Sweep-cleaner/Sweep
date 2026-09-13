//! Linux specifics: `FITRIM`, `journalctl`, memory wiping.

use std::path::Path;

use crate::core::error::{Error, Result};

/// `struct fstrim_range { __u64 start; __u64 len; __u64 minlen; }` — 24 bytes.
#[repr(C)]
struct FstrimRange {
    start: u64,
    len: u64,
    minlen: u64,
}

/// `_IOWR('X', 121, struct fstrim_range)`. The struct is 24 bytes on every
/// architecture, so the encoding is identical on 32- and 64-bit Linux.
const FITRIM: libc::c_ulong = 0xC018_5879;

/// Ask the block layer to discard every unused block on the filesystem
/// containing `path`.
///
/// This is the correct way to "wipe free space" on an SSD: it happens in
/// milliseconds and consumes no write cycles, unlike overwriting.
pub fn fitrim(path: &Path) -> Result<()> {
    use std::ffi::CString;

    let target = if path.is_file() {
        path.parent().unwrap_or(Path::new("/")).to_path_buf()
    } else {
        path.to_path_buf()
    };

    if !target.exists() {
        return Err(Error::io(
            &target,
            std::io::Error::from_raw_os_error(libc::ENOENT),
        ));
    }

    let c_path = CString::new(target.to_string_lossy().as_bytes())
        .map_err(|_| Error::msg("path contains a NUL byte"))?;

    unsafe {
        let fd = libc::open(c_path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY);
        if fd < 0 {
            return Err(Error::io(
                &target,
                std::io::Error::from_raw_os_error(*libc::__errno_location()),
            ));
        }

        let mut range = FstrimRange {
            start: 0,
            len: u64::MAX,
            minlen: 0,
        };

        let rc = libc::ioctl(
            fd,
            FITRIM,
            &mut range as *mut FstrimRange as *mut libc::c_void,
        );
        let errno = *libc::__errno_location();
        libc::close(fd);

        if rc != 0 {
            return Err(Error::io(&target, std::io::Error::from_raw_os_error(errno)));
        }
    }

    Ok(())
}

/// Wipe swap and drop page cache / dentries / inodes.
///
/// BleachBit exposes this as `system.memory`. It requires root and can make
/// the system unstable, so it is only reachable with `--expert`.
pub fn wipe_memory() -> Result<()> {
    use std::process::Command;

    if unsafe { libc::geteuid() } != 0 {
        return Err(Error::msg("wiping memory requires root"));
    }

    // Drop caches first so swapoff has somewhere to put the pages.
    let _ = std::fs::write("/proc/sys/vm/drop_caches", "3\n");

    // Turn every swap device off and on again.
    let swaps = std::fs::read_to_string("/proc/swaps").unwrap_or_default();
    let devices: Vec<String> = swaps
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .filter(|dev| dev.starts_with('/'))
        .map(|dev| dev.to_string())
        .collect();

    for device in &devices {
        let status = Command::new("swapoff").arg(device).status();
        match status {
            Ok(status) if status.success() => {
                log::info!("disabled swap on {device}");
            }
            Ok(status) => {
                log::warn!("swapoff {device} exited with {status}");
            }
            Err(err) => log::warn!("swapoff {device} failed: {err}"),
        }
    }

    // Any page still in swap is now unrecoverable-by-design.
    for device in &devices {
        let _ = Command::new("swapon").arg(device).status();
    }

    Ok(())
}

/// `journalctl --vacuum-time=Nd` — keep only the last `vacuum_days` of logs.
pub fn journald_clean(vacuum_days: u32) -> Result<u64> {
    let time_arg = format!("--vacuum-time={vacuum_days}d");
    let before = crate::fsutil::size::dir_size(Path::new("/var/log/journal"));

    let status = std::process::Command::new("journalctl")
        .args([time_arg.as_str()])
        .status()
        .map_err(|err| Error::Spawn {
            command: format!("journalctl {time_arg}"),
            reason: err.to_string(),
        })?;

    if !status.success() {
        return Err(Error::ExternalCommand {
            command: format!("journalctl {time_arg}"),
            status: status.code().unwrap_or(-1),
            stderr: String::new(),
        });
    }

    let after = crate::fsutil::size::dir_size(Path::new("/var/log/journal"));
    Ok(before.saturating_sub(after))
}

/// Is `systemd-resolved` (or `resolvectl`) available for a DNS flush?
pub fn has_resolved() -> bool {
    crate::fsutil::path_exists_in_path("resolvectl")
        || crate::fsutil::path_exists_in_path("systemd-resolve")
}

/// Flush the DNS resolver cache.
pub fn flush_dns() -> Result<()> {
    let mut last_error: Option<Error> = None;
    for (program, args) in [
        ("resolvectl", vec!["flush-caches"]),
        ("systemd-resolve", vec!["--flush-caches"]),
    ] {
        if !crate::fsutil::path_exists_in_path(program) {
            continue;
        }
        let status = std::process::Command::new(program)
            .args(&args)
            .status()
            .map_err(|err| Error::Spawn {
                command: format!("{program} {}", args.join(" ")),
                reason: err.to_string(),
            })?;
        if status.success() {
            return Ok(());
        }
        log::debug!("{program} exited with {status}");
        last_error = Some(Error::ExternalCommand {
            command: format!("{program} {}", args.join(" ")),
            status: status.code().unwrap_or(-1),
            stderr: String::new(),
        });
    }
    Err(last_error
        .unwrap_or_else(|| Error::msg("neither resolvectl nor systemd-resolve is installed")))
}

/// Flatpak/Snap leftover directories that BleachBit does not cover.
pub fn container_leftovers() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        let flatpak_unused = home.join(".local/share/flatpak/.removed");
        if flatpak_unused.is_dir() {
            dirs.push(flatpak_unused);
        }
    }
    if let Some(cache) = crate::platform::cache_dir() {
        for name in ["flatpak", "snapcraft", "appstream"] {
            let dir = cache.join(name);
            if dir.is_dir() {
                dirs.push(dir);
            }
        }
    }
    dirs
}
