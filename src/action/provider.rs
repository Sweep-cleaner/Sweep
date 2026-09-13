//! The action-provider registry.
//!
//! Every `<action command="...">` / `command = "..."` in a cleaner definition
//! names one of the providers below. Keeping them in a single table gives us
//! three things BleachBit does not have:
//!
//! * **load-time validation** — a typo in a cleaner is reported when the
//!   cleaner is parsed, not when it silently does nothing;
//! * **self-documentation** — `sweep providers` prints this table;
//! * **a security boundary** — the trust level decides which providers an
//!   untrusted cleaner may reach.

use crate::core::error::{Error, Result};

/// Static description of one action provider.
#[derive(Debug, Clone, Copy)]
pub struct ProviderInfo {
    /// Command name as written in cleaner definitions.
    pub name: &'static str,
    /// One-line explanation.
    pub summary: &'static str,
    /// Does the command require a `path`?
    pub needs_path: bool,
    /// Does it destroy data on disk (as opposed to editing a file in place)?
    pub destructive: bool,
    /// Is it only meaningful on Windows?
    pub windows_only: bool,
    /// Is it only meaningful on Unix?
    pub unix_only: bool,
    /// May an untrusted cleaner use it?
    pub privileged: bool,
}

impl ProviderInfo {
    /// Does this provider do anything on the running platform?
    pub fn available(self) -> bool {
        if self.windows_only && !cfg!(windows) {
            return false;
        }
        if self.unix_only && !cfg!(unix) {
            return false;
        }
        true
    }
}

/// Every action command Sweep understands.
pub const PROVIDERS: &[ProviderInfo] = &[
    // ---- filesystem --------------------------------------------------------
    ProviderInfo {
        name: "delete",
        summary: "Delete files or directories",
        needs_path: true,
        destructive: true,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "shred",
        summary: "Overwrite and then delete, regardless of the global shred setting",
        needs_path: true,
        destructive: true,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "truncate",
        summary: "Empty a file but keep it (for logs that must keep their inode)",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    // ---- structured files --------------------------------------------------
    ProviderInfo {
        name: "ini",
        summary: "Remove a section or parameter from an INI file",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "json",
        summary: "Remove a key from a JSON file",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    // ---- SQLite ------------------------------------------------------------
    ProviderInfo {
        name: "sqlite.vacuum",
        summary: "VACUUM a SQLite database to reclaim deleted pages",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "chrome.history",
        summary: "Delete browsing history from a Chromium database",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "chrome.keywords",
        summary: "Delete search-engine keywords (omnibox history) from Chromium",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "chrome.autofill",
        summary: "Delete autofill profiles from Chromium Web Data (cards kept)",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "chrome.favicons",
        summary: "Delete favicon cache from Chromium",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "chrome.databases_db",
        summary: "Clear the Chromium DatabaseTracker caches",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "chrome.cookies",
        summary: "Delete cookies from a Chromium database",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "mozilla.history",
        summary: "Delete Firefox/Mozilla browsing history",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "mozilla.url_history",
        summary: "Delete Firefox/Mozilla URL history and shrink places.sqlite",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "mozilla.databases",
        summary: "Clear the Firefox/Mozilla DatabaseTracker caches",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "mozilla.vacuum",
        summary: "VACUUM every SQLite database in a Mozilla profile",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "cookies",
        summary: "Delete cookies, auto-detecting Chromium vs. Mozilla schema",
        needs_path: true,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    // ---- Windows registry --------------------------------------------------
    ProviderInfo {
        name: "winreg",
        summary: "Delete a Windows registry key or value",
        needs_path: true,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: true,
    },
    // ---- external processes ------------------------------------------------
    ProviderInfo {
        name: "process",
        summary: "Run an external command",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: false,
        privileged: true,
    },
    // ---- system operations -------------------------------------------------
    ProviderInfo {
        name: "system.clipboard",
        summary: "Clear the clipboard",
        needs_path: false,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.dns",
        summary: "Flush the DNS resolver cache",
        needs_path: false,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.memory",
        summary: "Wipe freed memory (Linux only)",
        needs_path: false,
        destructive: false,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.trash",
        summary: "Empty the desktop trash / recycle bin",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.rotated_logs",
        summary: "Delete rotated and compressed log files in system log dirs",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.localizations",
        summary: "Delete unused localisation files, keeping the configured locales",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.recent_documents",
        summary: "Clear the recently-used document list",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.custom",
        summary: "Clean the user-configured custom paths",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.tmp",
        summary: "Delete stale files in the system temporary directory",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.free_disk_space",
        summary: "Overwrite free space on the volume containing this path",
        needs_path: true,
        destructive: true,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.journald",
        summary: "Vacuum the systemd journal",
        needs_path: false,
        destructive: false,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.font_cache",
        summary: "Rebuild the font cache",
        needs_path: false,
        destructive: false,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.recycle_bin",
        summary: "Empty the Windows recycle bin on every drive",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.prefetch",
        summary: "Delete Windows prefetch files",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.wer",
        summary: "Delete Windows error-reporting queues",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.windows_update_cache",
        summary: "Delete the Windows Update download cache",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.delivery_optimization",
        summary: "Delete the Windows Delivery Optimization cache",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.explorer_caches",
        summary: "Delete Windows Explorer thumbnail and icon caches",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.thumbcache",
        summary: "Delete Explorer thumbnail and icon database files (thumbcache_*.db)",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.store_cache",
        summary: "Delete the Microsoft Store download and update cache",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.d3d_shader_cache",
        summary: "Delete the per-user DirectX shader compilation cache",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.crash_dumps",
        summary: "Delete user-mode crash dumps",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.memory_dumps",
        summary: "Delete system crash memory dumps (MEMORY.DMP, Minidump)",
        needs_path: false,
        destructive: true,
        windows_only: true,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.trim",
        summary: "Issue TRIM/discard for the volume containing this path",
        needs_path: false,
        destructive: false,
        windows_only: false,
        unix_only: false,
        privileged: false,
    },
    ProviderInfo {
        name: "system.nix_gc",
        summary: "Nix store garbage collection (nix-collect-garbage -d)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.tm_thin",
        summary: "Thin Time Machine local snapshots (tmutil thinlocalsnapshots, macOS)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.docker_volumes",
        summary: "Remove dangling Docker volumes (docker volume prune)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.journal_user",
        summary: "Vacuum the per-user systemd journal (no root needed)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    // ---- package managers --------------------------------------------------
    ProviderInfo {
        name: "apt.clean",
        summary: "apt-get clean — remove downloaded .deb files",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "apt.autoclean",
        summary: "apt-get autoclean — remove obsolete .deb files",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "apt.autoremove",
        summary: "apt-get autoremove — remove unneeded packages (needs root)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: true,
    },
    ProviderInfo {
        name: "dnf.clean",
        summary: "dnf clean all",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "yum.clean",
        summary: "yum clean all",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "pacman.clean",
        summary: "pacman -Scc --noconfirm",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "zypper.clean",
        summary: "zypper clean --all",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    // ---- Linux deep-clean categories (each backed by src/deep/<name>.rs) ---
    ProviderInfo {
        name: "system.apt_cache",
        summary: "Deep APT cache scan with per-file size preview",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.dnf_cache",
        summary: "Deep DNF/YUM cache scan with per-file size preview",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.pacman_cache",
        summary: "Deep pacman cache scan (paccache -r keeps N versions)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.old_kernels",
        summary: "Remove old kernels, never the running one",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.journal_vacuum",
        summary: "Vacuum the systemd journal to a size/time budget",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.user_caches",
        summary: "Clean safe user/application caches with size report",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.temp_deep",
        summary: "Clean stale files in /tmp and /var/tmp",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.orphans",
        summary: "List and remove orphaned packages via the native manager",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.snap",
        summary: "Remove disabled Snap revisions and Snap caches",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.flatpak",
        summary: "Remove unused Flatpak runtimes and leftover dirs",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.docker",
        summary: "Prune dangling Docker images, build cache and container logs",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.kde_cache",
        summary: "Clean safe Plasma/desktop cache leaves with size report",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.kde_pim",
        summary: "Clean the Akonadi file cache (database untouched)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.baloo",
        summary: "Purge the Baloo file index via balooctl",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.gnome_cache",
        summary: "Clean safe GNOME app cache leaves with size report",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.zeitgeist",
        summary: "Clear the Zeitgeist activity log (privacy)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.tracker",
        summary: "Reset the Tracker file index via tracker3",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.apt_lists",
        summary: "Clear APT package lists (regenerated by apt-get update)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.zypp_cache",
        summary: "Deep zypper cache scan with per-file size preview",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.crash_reports",
        summary: "Delete apport/ABRT crash reports",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.coredumps",
        summary: "Empty the systemd-coredump store",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.dev_npm",
        summary: "Clean the npm download cache (~/.npm)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.dev_pip",
        summary: "Clean the pip cache (~/.cache/pip)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.dev_cargo",
        summary: "Clean the cargo registry cache (~/.cargo/registry)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.dev_gradle",
        summary: "Clean the Gradle caches (~/.gradle/caches)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "system.dev_go",
        summary: "Clean the Go build cache and module cache (~/.cache/go-build, ~/go/pkg/mod)",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
    ProviderInfo {
        name: "brew.cleanup",
        summary: "brew cleanup --prune=all",
        needs_path: false,
        destructive: true,
        windows_only: false,
        unix_only: true,
        privileged: false,
    },
];

/// Does a provider with this command name exist?
pub fn exists(command: &str) -> bool {
    PROVIDERS.iter().any(|p| p.name == command)
}

/// Look up a provider, failing with a helpful error.
pub fn lookup(command: &str) -> Result<&'static ProviderInfo> {
    PROVIDERS
        .iter()
        .find(|p| p.name == command)
        .ok_or_else(|| Error::msg(format!("unknown action command '{command}'")))
}

/// Every provider, for `sweep providers`.
pub fn list() -> &'static [ProviderInfo] {
    PROVIDERS
}

/// Is this action command forbidden to an untrusted cleaner — for example a
/// rule installed from the community hub?
///
/// This is the single source of truth for Sweep's trust boundary. Three
/// callers rely on it:
///
/// * [`crate::definition::model::apply_trust`] (native TOML + winapp2),
/// * the CleanerML importer, which also runs `apply_trust` afterwards,
/// * the hub installer's pre-install security scan.
///
/// It fails closed: a command that is not in the registry cannot be vouched
/// for, so it is treated as privileged. (Unknown commands are rejected at parse
/// time as well, so this is purely defence in depth — and it means a newly
/// added privileged provider is automatically blocked for untrusted sources
/// without any other code having to be updated.)
pub fn is_privileged(command: &str) -> bool {
    lookup(command).map_or(true, |p| p.privileged)
}

/// Does this provider interpret its `path` as a location on the filesystem?
///
/// Two providers do not: `winreg` takes a registry key (`HKCU\Software\…`)
/// and `process` a command line. Both are therefore exempt from the
/// "cleaner paths must be absolute" rule that guards every other provider.
pub fn filesystem_path(command: &str) -> bool {
    !matches!(command, "winreg" | "process")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_is_unique() {
        let mut seen = std::collections::HashSet::new();
        for provider in PROVIDERS {
            assert!(
                seen.insert(provider.name),
                "duplicate provider '{}'",
                provider.name
            );
        }
    }

    #[test]
    fn lookup_works() {
        assert!(lookup("delete").is_ok());
        assert!(lookup("nope").is_err());
        assert!(exists("sqlite.vacuum"));
    }

    #[test]
    fn privileged_flag_drives_trust() {
        // process/winreg are the classic privileged providers.
        assert!(is_privileged("process"));
        assert!(is_privileged("winreg"));
        // apt.autoremove needs root and must not run for untrusted sources.
        assert!(is_privileged("apt.autoremove"));
        // Ordinary filesystem providers are fine for anyone.
        assert!(!is_privileged("delete"));
        assert!(!is_privileged("truncate"));
        // Unknown commands fail closed (treated as privileged).
        assert!(is_privileged("does-not-exist"));
    }
}
