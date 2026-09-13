//! Windows specifics.
//!
//! The handful of Win32 calls we need are declared directly with
//! `extern "system"` instead of pulling in a bindings crate. That keeps the
//! dependency tree (and the binary) small and avoids the feature-flag
//! gymnastics of the `windows-sys` metapackage.

#![allow(non_snake_case)]

use std::ffi::{OsStr, OsString};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use crate::core::error::{Error, Result};

// ---------------------------------------------------------------------------
// Raw Win32 declarations
// ---------------------------------------------------------------------------

const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFF_FFFF;
const INVALID_HANDLE_VALUE: isize = -1;
const FILE_ATTRIBUTE_READONLY: u32 = 0x0000_0001;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
const MOVEFILE_DELAY_UNTIL_REBOOT: u32 = 0x0000_0004;
const FILE_SHARE_READ: u32 = 0x0000_0001;
const FILE_SHARE_WRITE: u32 = 0x0000_0002;
const FILE_SHARE_DELETE: u32 = 0x0000_0004;
const OPEN_EXISTING: u32 = 3;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const CP_UTF8: u32 = 65_001;

/// `BY_HANDLE_FILE_INFORMATION`; `FILETIME` is two `DWORD`s.
#[repr(C)]
struct ByHandleFileInformation {
    file_attributes: u32,
    creation_time: [u32; 2],
    last_access_time: [u32; 2],
    last_write_time: [u32; 2],
    volume_serial_number: u32,
    file_size_high: u32,
    file_size_low: u32,
    number_of_links: u32,
    file_index_high: u32,
    file_index_low: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn GetFileAttributesW(path: *const u16) -> u32;
    fn SetFileAttributesW(path: *const u16, attrs: u32) -> i32;
    fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    fn GetDiskFreeSpaceExW(
        path: *const u16,
        free_available: *mut u64,
        total: *mut u64,
        total_free: *mut u64,
    ) -> i32;
    fn CreateFileW(
        file_name: *const u16,
        desired_access: u32,
        share_mode: u32,
        security_attributes: *mut std::ffi::c_void,
        creation_disposition: u32,
        flags_and_attributes: u32,
        template_file: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn GetFileInformationByHandle(
        handle: *mut std::ffi::c_void,
        info: *mut ByHandleFileInformation,
    ) -> i32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn EmptyClipboard() -> i32;
    fn OpenClipboard(owner: *mut std::ffi::c_void) -> i32;
    fn CloseClipboard() -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn SetConsoleOutputCP(code_page: u32) -> i32;
    fn SetConsoleCP(code_page: u32) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn SHChangeNotify(
        event_id: i32,
        flags: u32,
        item1: *const std::ffi::c_void,
        item2: *const std::ffi::c_void,
    );
    fn SHEmptyRecycleBinW(hwnd: *mut std::ffi::c_void, root: *const u16, flags: u32) -> i32;
}

#[link(name = "msvcrt")]
extern "system" {
    fn _flushall() -> i32;
}

const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
const SHCNE_UPDATEDIR: i32 = 0x0000_1000;
const SHCNF_IDLIST: u32 = 0x0000;
const SHCNF_PATHW: u32 = 0x0005;
const SHERB_NOCONFIRMATION: u32 = 0x0000_0001;
const SHERB_NOPROGRESSUI: u32 = 0x0000_0002;
const SHERB_NOSOUND: u32 = 0x0000_0004;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a path to a NUL-terminated UTF-16 buffer.
fn to_wide(path: &Path) -> Vec<u16> {
    let text = crate::core::path::extended_path(path);
    let mut wide: Vec<u16> = OsStr::new(&text).encode_wide().collect();
    wide.push(0);
    wide
}

fn from_wide(wide: &[u16]) -> OsString {
    OsString::from_wide(wide)
}

/// Win32 attributes for `path`, or `None` when it does not exist.
pub fn file_attributes(path: &Path) -> Option<u32> {
    let wide = to_wide(path);
    unsafe {
        let attrs = GetFileAttributesW(wide.as_ptr());
        if attrs == INVALID_FILE_ATTRIBUTES {
            None
        } else {
            Some(attrs)
        }
    }
}

/// Is `path` a reparse point (symlink or junction)?
pub fn is_reparse_point(path: &Path) -> bool {
    matches!(
        file_attributes(path),
        Some(attrs) if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0
    )
}

/// Is `path` a mount point (volume root or junction)?
pub fn is_mount_point(path: &Path) -> bool {
    if is_reparse_point(path) {
        return true;
    }
    // Compare the volume GUID path of `path` with that of its parent: when
    // they differ, `path` is a mounted volume.
    let Some(own) = volume_path(path) else {
        return false;
    };
    match path.parent().and_then(volume_path) {
        Some(parent) => own != parent,
        None => true,
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn GetVolumePathNameW(path: *const u16, volume: *mut u16, size: u32) -> i32;
}

fn volume_path(path: &Path) -> Option<String> {
    let wide = to_wide(path);
    let mut buffer = [0u16; 32768];
    unsafe {
        if GetVolumePathNameW(wide.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) != 0 {
            let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
            Some(wide_to_string(&buffer[..end]))
        } else {
            None
        }
    }
}

/// Does `path` have more than one hard link?
///
/// A shredded hard link is a data-loss bug: overwriting the file's content
/// destroys the bytes every *other* name for that file points at, and the
/// other names survive the unlink. So the shredder must see the real link
/// count. `GetFileInformationByHandle` reports it (`nNumberOfLinks`) and is a
/// stable kernel32 API, so no nightly `windows_by_handle` feature is needed.
pub fn is_hard_link(path: &Path) -> bool {
    let wide = to_wide(path);
    unsafe {
        // Asking for no access is enough to query metadata, and it succeeds
        // on files we are not allowed to read — which is exactly when we most
        // want the answer. The reparse flag keeps us from following a link.
        let handle = CreateFileW(
            wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        );
        if handle as isize == INVALID_HANDLE_VALUE {
            // Cannot open it: report "not a hard link" rather than blocking a
            // delete. The path is unlinked either way; only shredding is
            // skipped, and the caller decides what to do with a locked file.
            return false;
        }
        let mut info: ByHandleFileInformation = std::mem::zeroed();
        let ok = GetFileInformationByHandle(handle, &mut info);
        CloseHandle(handle);
        ok != 0
            && info.file_attributes & FILE_ATTRIBUTE_DIRECTORY == 0
            && info.number_of_links > 1
    }
}

/// Remove the read-only attribute. Returns `true` when it was present.
pub fn clear_readonly(path: &Path) -> bool {
    match file_attributes(path) {
        Some(attrs) if attrs & FILE_ATTRIBUTE_READONLY != 0 => {
            let wide = to_wide(path);
            unsafe { SetFileAttributesW(wide.as_ptr(), attrs & !FILE_ATTRIBUTE_READONLY) != 0 }
        }
        _ => false,
    }
}

/// Schedule a file for deletion at the next reboot.
///
/// Used for files that are locked by another process and cannot even be
/// truncated — BleachBit's `delete_locked_file()`.
pub fn delete_on_reboot(path: &Path) -> Result<()> {
    let wide = to_wide(path);
    unsafe {
        if MoveFileExW(wide.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT) == 0 {
            return Err(Error::io(
                path,
                std::io::Error::from_raw_os_error(
                    std::io::Error::last_os_error().raw_os_error().unwrap_or(5),
                ),
            ));
        }
    }
    Ok(())
}

/// Flush all C runtime file buffers.
pub fn flush_all() {
    unsafe {
        _flushall();
    }
}

/// Put the attached console into UTF-8 mode.
///
/// Sweep prints cleaner labels, file paths and its own messages, and plenty of
/// those are not ASCII. Rust writes UTF-8 to stdout; without this the console
/// decodes those bytes with the legacy ANSI code page and the text arrives
/// mangled. Both calls fail harmlessly when there is no console (a GUI launch,
/// a service) or when output is redirected to a file or pipe, where the bytes
/// are already UTF-8.
pub fn init_console() {
    unsafe {
        SetConsoleOutputCP(CP_UTF8);
        SetConsoleCP(CP_UTF8);
    }
}

// ---------------------------------------------------------------------------
// Disk space
// ---------------------------------------------------------------------------

/// Free space available to the calling user.
pub fn free_space(path: &Path) -> Result<u64> {
    let wide = to_wide(path);
    let mut available: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;
    unsafe {
        if GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut total_free) == 0 {
            return Err(Error::io(
                path,
                std::io::Error::from_raw_os_error(
                    std::io::Error::last_os_error().raw_os_error().unwrap_or(5),
                ),
            ));
        }
    }
    Ok(available)
}

/// Total capacity of the volume.
pub fn total_space(path: &Path) -> Result<u64> {
    let wide = to_wide(path);
    let mut available: u64 = 0;
    let mut total: u64 = 0;
    let mut total_free: u64 = 0;
    unsafe {
        if GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut total_free) == 0 {
            return Err(Error::io(
                path,
                std::io::Error::from_raw_os_error(
                    std::io::Error::last_os_error().raw_os_error().unwrap_or(5),
                ),
            ));
        }
    }
    Ok(total)
}

/// Ask the volume to trim unused blocks.
///
/// Windows does this automatically as part of its "Optimize Drives"
/// maintenance; there is no documented per-path ioctl, so we report
/// unsupported and let the caller fall back to overwriting.
pub fn trim(_path: &Path) -> Result<()> {
    Err(Error::msg(
        "Windows performs TRIM automatically; use overwriting for an immediate wipe",
    ))
}

// ---------------------------------------------------------------------------
// Shell
// ---------------------------------------------------------------------------

/// Tell Explorer that the namespace changed so icons refresh.
pub fn shell_change_notify() {
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

/// Refresh a specific directory in Explorer.
pub fn shell_refresh_dir(path: &Path) {
    let wide = to_wide(path);
    unsafe {
        SHChangeNotify(
            SHCNE_UPDATEDIR,
            SHCNF_PATHW,
            wide.as_ptr() as *const std::ffi::c_void,
            std::ptr::null(),
        );
    }
}

/// Directories that make up the recycle bin.
///
/// Modern Windows keeps one `$Recycle.Bin\<SID>` folder per volume; the legacy
/// `RECYCLER`/`Recycled` names are still checked for completeness.
pub fn recycle_bin_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());

    for name in ["$Recycle.Bin", "RECYCLER", "Recycled"] {
        let candidate = PathBuf::from(format!("{drive}\\")).join(name);
        if candidate.is_dir() {
            // Per-user SID subdirectory keeps us inside our own bin.
            if let Some(sid) = current_user_sid() {
                let scoped = candidate.join(&sid);
                if scoped.is_dir() {
                    dirs.push(scoped);
                    continue;
                }
            }
            dirs.push(candidate);
        }
    }
    dirs
}

fn current_user_sid() -> Option<String> {
    std::process::Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .ok()
        .and_then(|out| {
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            text.split(',')
                .nth(1)
                .map(|s| s.trim_matches('"').trim().to_string())
        })
        .filter(|s| s.starts_with("S-1-"))
}

/// Empty the recycle bin through the shell API.
pub fn empty_recycle_bin() -> Result<()> {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let root = format!("{drive}\\");
    let wide: Vec<u16> = OsStr::new(&root)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let flags = SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND;
    unsafe {
        if SHEmptyRecycleBinW(std::ptr::null_mut(), wide.as_ptr(), flags) != 0 {
            return Ok(());
        }
    }
    Err(Error::msg("SHEmptyRecycleBinW failed"))
}

/// Clear the clipboard.
pub fn clear_clipboard() -> Result<()> {
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err(Error::msg("OpenClipboard failed"));
        }
        let rc = EmptyClipboard();
        CloseClipboard();
        if rc == 0 {
            return Err(Error::msg("EmptyClipboard failed"));
        }
    }
    Ok(())
}

/// `ipconfig /flushdns`.
pub fn flush_dns() -> Result<()> {
    let status = std::process::Command::new("ipconfig")
        .arg("/flushdns")
        .status()
        .map_err(|err| Error::Spawn {
            command: "ipconfig /flushdns".into(),
            reason: err.to_string(),
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::ExternalCommand {
            command: "ipconfig /flushdns".into(),
            status: status.code().unwrap_or(-1),
            stderr: String::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// Well-known locations
// ---------------------------------------------------------------------------

/// `%WINDIR%\Prefetch`.
pub fn prefetch_dir() -> Option<PathBuf> {
    std::env::var_os("windir").map(|w| PathBuf::from(w).join("Prefetch"))
}

/// Windows Error Reporting queues and archives.
pub fn wer_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(
            PathBuf::from(&local)
                .join("Microsoft")
                .join("Windows")
                .join("WER"),
        );
    }
    if let Ok(program_data) = std::env::var("ProgramData") {
        dirs.push(
            PathBuf::from(&program_data)
                .join("Microsoft")
                .join("Windows")
                .join("WER"),
        );
    }
    dirs.into_iter().filter(|d| d.is_dir()).collect()
}

/// `SoftwareDistribution\Download` — the Windows Update cache.
pub fn windows_update_cache() -> Option<PathBuf> {
    std::env::var("windir")
        .ok()
        .map(|w| {
            PathBuf::from(w)
                .join("SoftwareDistribution")
                .join("Download")
        })
        .filter(|p| p.is_dir())
}

/// Delivery Optimization cache, which can grow to tens of gigabytes.
pub fn delivery_optimization_cache() -> Option<PathBuf> {
    std::env::var("windir")
        .ok()
        .map(|w| {
            PathBuf::from(w)
                .join("SoftwareDistribution")
                .join("DeliveryOptimization")
        })
        .filter(|p| p.is_dir())
}

/// Thumbnail / icon caches kept per user.
pub fn explorer_caches() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let explorer = PathBuf::from(&local)
            .join("Microsoft")
            .join("Windows")
            .join("Explorer");
        dirs.push(explorer.join("ThumbCacheToDelete"));
        dirs.push(explorer);
    }
    dirs.into_iter().filter(|d| d.is_dir()).collect()
}

/// Explorer thumbnail / icon database files
/// (`%LOCALAPPDATA%\Microsoft\Windows\Explorer\thumbcache_*.db`).
///
/// Unlike [`explorer_caches`] this targets only the database files, not the
/// whole Explorer directory, so a locked icon-cache database cannot silently
/// neuter the rest of the cleanup.
pub fn explorer_thumb_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(&local)
            .join("Microsoft")
            .join("Windows")
            .join("Explorer");
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if (name.starts_with("thumbcache_") || name.starts_with("iconcache_"))
                    && name.ends_with(".db")
                {
                    out.push(entry.path());
                }
            }
        }
    }
    out
}

/// Microsoft Store per-user package cache (`LocalCache` of the Store package).
pub fn store_cache_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(&local)
            .join("Packages")
            .join("Microsoft.WindowsStore_8wekyb3d8bbwe")
            .join("LocalCache");
        if dir.is_dir() {
            out.push(dir);
        }
    }
    out
}

/// Per-user DirectX shader compilation cache (`%LOCALAPPDATA%\D3DSCache`).
/// The GPU driver rebuilds it on demand.
pub fn d3d_shader_cache_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(&local).join("D3DSCache");
        if dir.is_dir() {
            out.push(dir);
        }
    }
    out
}

/// User-mode crash dumps (`%LOCALAPPDATA%\CrashDumps`).
pub fn crash_dump_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(&local).join("CrashDumps");
        if dir.is_dir() {
            out.push(dir);
        }
    }
    out
}

/// System crash memory dumps: `%WINDIR%\MEMORY.DMP` and `%WINDIR%\Minidump`.
pub fn memory_dumps() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(windir) = std::env::var("windir") {
        let windir = PathBuf::from(windir);
        let dump = windir.join("MEMORY.DMP");
        if dump.exists() {
            out.push(dump);
        }
        let minidump = windir.join("Minidump");
        if minidump.is_dir() {
            out.push(minidump);
        }
    }
    out
}

/// Directories that hold uninstallers for Windows updates / hotfixes.
pub fn update_uninstall_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(windir) = std::env::var("windir") {
        let windir = PathBuf::from(windir);
        for name in [
            "$NtUninstallKB*",
            "$NtServicePackUninstall*",
            "$NtUninstall*",
        ] {
            if let Ok(entries) = std::fs::read_dir(&windir) {
                for entry in entries.flatten() {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if glob_like(name, &file_name) && entry.path().is_dir() {
                        dirs.push(entry.path());
                    }
                }
            }
        }
    }
    dirs
}

/// Minimal `*` glob matcher, enough for the fixed patterns above.
fn glob_like(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    match parts.as_slice() {
        [] => true,
        [single] => text == *single,
        [first, rest @ ..] => {
            if !text.starts_with(first) {
                return false;
            }
            let mut tail = &text[first.len()..];
            for part in &rest[..rest.len().saturating_sub(1)] {
                match tail.find(part) {
                    Some(idx) => tail = &tail[idx + part.len()..],
                    None => return false,
                }
            }
            if let Some(last) = rest.last() {
                if last.is_empty() {
                    true
                } else {
                    tail.ends_with(*last)
                }
            } else {
                true
            }
        }
    }
}

/// `sweep doctor` extras.
pub fn diagnostics() -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(windir) = std::env::var("windir") {
        out.push(format!("windir: {windir}"));
    }
    out.push(format!(
        "recycle bin: {} entr(ies)",
        recycle_bin_dirs().len()
    ));
    out
}

/// Decode a UTF-16 buffer produced by the registry helpers.
pub fn wide_to_string(wide: &[u16]) -> String {
    from_wide(wide).to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

/// Registry hives accepted in cleaner definitions.
///
/// We name the hive ourselves instead of storing a raw `HKEY` so that the
/// (version-dependent) type never has to be spelled out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hive {
    /// `HKEY_CURRENT_USER` / `HKCU`.
    CurrentUser,
    /// `HKEY_LOCAL_MACHINE` / `HKLM`.
    LocalMachine,
    /// `HKEY_CLASSES_ROOT` / `HKCR`.
    ClassesRoot,
    /// `HKEY_USERS` / `HKU`.
    Users,
}

fn hive_from_name(name: &str) -> Option<Hive> {
    match name.to_ascii_uppercase().as_str() {
        "HKCU" | "HKEY_CURRENT_USER" => Some(Hive::CurrentUser),
        "HKLM" | "HKEY_LOCAL_MACHINE" => Some(Hive::LocalMachine),
        "HKCR" | "HKEY_CLASSES_ROOT" => Some(Hive::ClassesRoot),
        "HKU" | "HKEY_USERS" => Some(Hive::Users),
        _ => None,
    }
}

fn open_root(hive: Hive) -> winreg::RegKey {
    use winreg::enums::*;
    match hive {
        Hive::CurrentUser => winreg::RegKey::predef(HKEY_CURRENT_USER),
        Hive::LocalMachine => winreg::RegKey::predef(HKEY_LOCAL_MACHINE),
        Hive::ClassesRoot => winreg::RegKey::predef(HKEY_CLASSES_ROOT),
        Hive::Users => winreg::RegKey::predef(HKEY_USERS),
    }
}

/// Split `HKCU\Software\Foo` into `(hive, "Software\Foo")`.
fn split_key(full: &str) -> Option<(Hive, String)> {
    let (hive, rest) = full.split_once('\\')?;
    let hive = hive_from_name(hive)?;
    let rest = rest.trim_matches('\\').to_string();
    if rest.is_empty() {
        None
    } else {
        Some((hive, rest))
    }
}

/// Read a string value out of the registry (used by `<var search="winreg">`).
pub fn reg_read_string(full: &str, name: Option<&str>) -> Option<String> {
    let (hive, subkey) = split_key(full)?;
    let key = open_root(hive).open_subkey(&subkey).ok()?;
    match name {
        Some(name) if !name.is_empty() => key.get_value::<String, _>(name).ok(),
        // No value name: return the key's default value.
        _ => key.get_value::<String, _>("").ok(),
    }
}

/// Delete an entire registry key, preserving `exclude` subkeys.
///
/// Returns `false` when the key does not exist (which is what makes
/// auto-hide work: a cleaner that has nothing to do stays hidden).
pub fn reg_delete_key(full: &str, exclude: &[String], really_delete: bool) -> Result<bool> {
    let (hive, subkey) = match split_key(full) {
        Some(parts) => parts,
        None => return Ok(false),
    };

    let (parent_path, leaf) = match subkey.rsplit_once('\\') {
        Some((parent, leaf)) => (parent.to_string(), leaf.to_string()),
        // Never delete a top-level hive key.
        None => return Ok(false),
    };

    let parent = open_root(hive)
        .open_subkey_with_flags(&parent_path, winreg::enums::KEY_READ)
        .map_err(|err| Error::msg(format!("{full}: {err}")))?;

    if parent.open_subkey(&leaf).is_err() {
        return Ok(false);
    }

    if !really_delete {
        return Ok(true);
    }

    let excluded: Vec<String> = exclude.iter().map(|e| e.to_lowercase()).collect();

    if excluded.is_empty() {
        parent
            .delete_subkey_all(&leaf)
            .map_err(|err| Error::msg(format!("{full}: {err}")))?;
    } else {
        // Remove the non-excluded children one by one, then the key itself.
        let target = parent
            .open_subkey_with_flags(&leaf, winreg::enums::KEY_ALL_ACCESS)
            .map_err(|err| Error::msg(format!("{full}: {err}")))?;
        let children: Vec<String> = target
            .enum_keys()
            .filter_map(|c| c.ok())
            .filter(|child| !excluded.contains(&child.to_lowercase()))
            .collect();
        for child in children {
            let _ = target.delete_subkey_all(child);
        }
        drop(target);
        if parent
            .open_subkey(&leaf)
            .map(|k| k.enum_keys().count() == 0)
            .unwrap_or(false)
        {
            parent
                .delete_subkey(&leaf)
                .map_err(|err| Error::msg(format!("{full}: {err}")))?;
        }
    }

    Ok(true)
}

/// Delete a single registry value.
pub fn reg_delete_value(full: &str, name: &str, really_delete: bool) -> Result<bool> {
    let (hive, subkey) = match split_key(full) {
        Some(parts) => parts,
        None => return Ok(false),
    };

    let key = match open_root(hive).open_subkey_with_flags(&subkey, winreg::enums::KEY_READ) {
        Ok(key) => key,
        Err(_) => return Ok(false),
    };

    if key.get_raw_value(name).is_err() {
        return Ok(false);
    }
    if !really_delete {
        return Ok(true);
    }

    let key = open_root(hive)
        .open_subkey_with_flags(&subkey, winreg::enums::KEY_SET_VALUE)
        .map_err(|err| Error::msg(format!("{full}: {err}")))?;
    key.delete_value(name)
        .map_err(|err| Error::msg(format!("{full}<{name}>: {err}")))?;
    Ok(true)
}

/// Does this registry key exist?
pub fn reg_key_exists(full: &str) -> bool {
    split_key(full)
        .and_then(|(hive, subkey)| open_root(hive).open_subkey(subkey).ok())
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sweep-win-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_single_name_is_not_a_hard_link() {
        let dir = scratch("single");
        let file = dir.join("plain.txt");
        std::fs::write(&file, b"data").unwrap();
        assert!(!is_hard_link(&file));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_name_is_reported_as_a_hard_link() {
        let dir = scratch("link");
        let first = dir.join("first.txt");
        let second = dir.join("second.txt");
        std::fs::write(&first, b"shared bytes").unwrap();
        std::fs::hard_link(&first, &second).unwrap();
        // Both names address the same content, so shredding either one would
        // destroy the other's data — the shredder has to see that.
        assert!(is_hard_link(&first));
        assert!(is_hard_link(&second));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn directories_are_not_hard_links() {
        let dir = scratch("dir");
        let nested = dir.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        assert!(!is_hard_link(&nested));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_path_is_not_a_hard_link() {
        let dir = scratch("missing");
        assert!(!is_hard_link(&dir.join("nothing-here")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_ordinary_file_is_not_a_reparse_point() {
        let dir = scratch("reparse");
        let file = dir.join("plain.txt");
        std::fs::write(&file, b"data").unwrap();
        assert!(!is_reparse_point(&file));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn readonly_attribute_round_trips() {
        let dir = scratch("readonly");
        let file = dir.join("locked.txt");
        std::fs::write(&file, b"data").unwrap();
        assert!(!clear_readonly(&file), "a fresh file is not read-only");
        let wide = to_wide(&file);
        unsafe {
            let attrs = GetFileAttributesW(wide.as_ptr());
            assert!(attrs != INVALID_FILE_ATTRIBUTES);
            assert!(SetFileAttributesW(wide.as_ptr(), attrs | FILE_ATTRIBUTE_READONLY) != 0);
        }
        assert!(clear_readonly(&file), "the read-only bit was set");
        assert!(!clear_readonly(&file), "and cleared again");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
