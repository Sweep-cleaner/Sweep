//! macOS specifics.

use std::path::{Path, PathBuf};

/// `~/Library/Caches` — the primary cache location on macOS.
pub fn user_caches() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        dirs.push(home.join("Library").join("Caches"));
    }
    dirs
}

/// `/Library/Caches` and `/System/Library/Caches` (system-wide, root only).
pub fn system_caches() -> Vec<PathBuf> {
    [
        PathBuf::from("/Library/Caches"),
        PathBuf::from("/System/Library/Caches"),
    ]
    .into_iter()
    .filter(|p| p.is_dir())
    .collect()
}

/// Per-user and system log directories.
pub fn log_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/private/var/log"), PathBuf::from("/var/log")];
    if let Some(home) = crate::platform::home_dir() {
        dirs.push(home.join("Library").join("Logs"));
    }
    dirs.into_iter().filter(|p| p.is_dir()).collect()
}

/// `.DS_Store` files and AppleDouble `._*` files are pure metadata noise.
pub fn junk_filenames() -> &'static [&'static str] {
    &[
        ".DS_Store",
        "._.DS_Store",
        ".Spotlight-V100",
        ".Trashes",
        ".fseventsd",
        ".TemporaryItems",
        "com.apple.timemachine.donotpresent",
    ]
}

/// Is this filename one of the Apple metadata artefacts?
pub fn is_apple_junk(path: &Path) -> bool {
    let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
        return false;
    };
    junk_filenames().contains(&name.as_str()) || name.starts_with("._")
}

/// `~/Library/Saved Application State` — per-application resume state.
pub fn saved_state_dir() -> Option<PathBuf> {
    crate::platform::home_dir().map(|home| home.join("Library").join("Saved Application State"))
}

/// QuickLook thumbnail cache.
pub fn quicklook_cache() -> Option<PathBuf> {
    crate::platform::home_dir().map(|home| {
        home.join("Library")
            .join("Caches")
            .join("com.apple.QuickLook.thumbnailcache")
    })
}
