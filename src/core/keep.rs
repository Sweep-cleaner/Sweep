//! The safety net: keep list, protected paths and the destructive-guard.
//!
//! BleachBit has two overlapping mechanisms — the per-cleaner `whitelist()` and
//! the global "whitelist paths" preference — plus an implicit rule that you
//! must never delete `/`, `C:\Windows` and friends. Sweep keeps the same
//! idea but makes it impossible to bypass by construction: every delete,
//! truncate and shred goes through [`Guard::check`].

use std::path::{Path, PathBuf};

use crate::core::error::{Error, Result};
use crate::core::path::{path_equal, path_equal_cs, path_starts_with_cs};
use crate::platform;

/// A user-configurable set of paths that must never be touched.
#[derive(Debug, Default, Clone)]
pub struct KeepList {
    files: Vec<PathBuf>,
    folders: Vec<PathBuf>,
}

impl KeepList {
    /// Empty keep list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Protect a single file.
    pub fn add_file<P: Into<PathBuf>>(&mut self, path: P) {
        self.files.push(expand(path.into()));
    }

    /// Protect a directory *and everything below it*.
    pub fn add_folder<P: Into<PathBuf>>(&mut self, path: P) {
        self.folders.push(expand(path.into()));
    }

    /// Number of entries (files + folders).
    pub fn len(&self) -> usize {
        self.files.len() + self.folders.len()
    }

    /// Is the keep list empty? Fast-paths the whole check in the hot loop.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.folders.is_empty()
    }

    /// Does `path` (or any of its parents) match a kept entry?
    pub fn contains(&self, path: &Path) -> bool {
        self.contains_cs(path, crate::core::path::case_sensitive())
    }

    /// [`KeepList::contains`] with an explicit case-sensitivity switch.
    pub fn contains_cs(&self, path: &Path, case_sensitive: bool) -> bool {
        self.matches_normalized(&crate::core::path::normalize(path), case_sensitive)
    }

    /// The same test for a path the caller has already normalised.
    ///
    /// [`Guard::check`] normalises each candidate once and reuses the result for
    /// the keep list and the protected roots; on a walk over a full cache tree
    /// that saves an allocation plus a component pass per path.
    pub fn matches_normalized(&self, path: &Path, case_sensitive: bool) -> bool {
        for file in &self.files {
            if path_equal_cs(path, file, case_sensitive) {
                return true;
            }
        }
        for folder in &self.folders {
            if path_starts_with_cs(path, folder, case_sensitive) {
                return true;
            }
        }
        false
    }

    /// Iterates the protected folders (used by the `doctor` command).
    pub fn folders(&self) -> impl Iterator<Item = &PathBuf> {
        self.folders.iter()
    }

    /// Iterates the protected files.
    pub fn files(&self) -> impl Iterator<Item = &PathBuf> {
        self.files.iter()
    }
}

fn expand(path: PathBuf) -> PathBuf {
    crate::core::path::normalize(&crate::core::path::expand(&path.to_string_lossy()))
}

/// System and home roots that are protected **as a node only**: the path itself
/// may never be deleted, but its contents are legitimately cleaned by dedicated
/// providers. The deeper [`crate::deep::safety`] layer recursively guards the
/// genuinely dangerous *subtrees* (`/etc`, `/usr/bin`, …) via an allowlist, so
/// we deliberately do not subtree-protect them here — doing so would block
/// legitimate cleanup such as `/var/log/journal` or `/usr/share/doc`.
///
/// User data directories that must keep *all* of their contents live in
/// [`user_data_subtree_roots`] instead.
pub fn protected_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();

    if let Some(home) = platform::home_dir() {
        // The home directory itself must survive even though everything
        // inside it is fair game.
        roots.push(home.clone());
        // These two are cleaner *targets* (Chromium/Firefox caches, …), so only
        // the node is protected — cleaners legitimately clean inside them.
        roots.push(home.join(".config"));
        roots.push(home.join(".local").join("share"));
    }

    if cfg!(windows) {
        for var in [
            "SystemDrive",
            "windir",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramW6432",
            "CommonProgramFiles",
            "ALLUSERSPROFILE",
            "USERPROFILE",
            "PUBLIC",
            "APPDATA",
            "LOCALAPPDATA",
        ] {
            if let Ok(value) = std::env::var(var) {
                let path = PathBuf::from(&value);
                if !path.as_os_str().is_empty() && !roots.iter().any(|r| path_equal(r, &path)) {
                    roots.push(path);
                }
            }
        }
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        let root = PathBuf::from(format!("{}\\", system_drive));
        if !roots.iter().any(|r| path_equal(r, &root)) {
            roots.push(root);
        }
    } else {
        for path in [
            "/",
            "/bin",
            "/boot",
            "/dev",
            "/etc",
            "/lib",
            "/lib32",
            "/lib64",
            "/libx32",
            "/proc",
            "/root",
            "/run",
            "/sbin",
            "/srv",
            "/sys",
            "/usr",
            "/var",
            "/var/lib",
            "/var/db",
            "/opt",
            "/System",
            "/Library",
            "/Applications",
            "/Network",
            "/private",
        ] {
            roots.push(PathBuf::from(path));
        }
    }

    roots.retain(|p| !p.as_os_str().is_empty());
    roots
}

/// User data directories that must survive **and keep every file inside them**.
///
/// A malicious or careless cleaner pointing at `~/Documents` or `~/.ssh` must
/// not be able to wipe a user's files: these are protected as a *subtree*, so
/// both the directory and anything beneath it are rejected. No built-in cleaner
/// legitimately targets these directories, so subtree protection is pure upside
/// (and it closes a real gap — previously only the exact node was protected, so
/// `~/.ssh/id_rsa` or `~/Documents/thesis.pdf` could be deleted).
pub fn user_data_subtree_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = platform::home_dir() {
        for leaf in [
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Music",
            "Videos",
            ".ssh",
            ".gnupg",
        ] {
            roots.push(home.join(leaf));
        }
    }
    roots
}

/// Paths that BleachBit's `System` cleaner keeps by convention, because the
/// applications that own them are covered by a dedicated cleaner or because
/// deleting them breaks the running session.
pub fn builtin_keep_list() -> KeepList {
    let mut list = KeepList::new();
    let home = platform::home_dir();

    if !cfg!(windows) {
        for folder in [
            "/tmp/.X11-unix",
            "/tmp/.font-unix",
            "/tmp/.ICE-unix",
            "/tmp/.Test-unix",
            "/tmp/.XIM-unix",
            "/tmp/systemd-private-*",
        ] {
            list.add_folder(PathBuf::from(folder));
        }
        list.add_file(PathBuf::from("/tmp/.X0-lock"));
        if let Some(home) = home.as_ref() {
            // Only runtime state survives here (sockets, mounts, live GPU
            // caches): disposable app caches are owned by their dedicated
            // cleaners and must NOT be keep-listed, otherwise the guard
            // — which cannot tell a dedicated cleaner from a generic walk —
            // would silently neuter firefox.cache, thumbnails.cache,
            // kde_cache, gnome_cache and friends (proven by benchmark).
            for folder in [
                ".cache/doc",
                ".cache/obexd",
                ".cache/ibus",
                ".cache/wallpaper",
                ".cache/mesa_shader_cache",
                ".cache/mesa_shader_cache_db",
                ".cache/drkonqi",
            ] {
                list.add_folder(home.join(folder));
            }
        }
    } else {
        // On Windows the temp *roots* themselves must stay: applications
        // recreate them, and removing them breaks installers.
        let windir = std::env::var("windir").unwrap_or_else(|_| r"C:\Windows".into());
        list.add_folder(PathBuf::from(windir).join("Temp"));
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            list.add_folder(PathBuf::from(local).join("Temp"));
        }
    }

    list
}

/// The runtime combination of the user keep list and the immutable protected
/// roots. Every destructive command consults it exactly once.
#[derive(Debug, Clone)]
pub struct Guard {
    keep: KeepList,
    protected: Vec<PathBuf>,
    /// User-data directories protected as a whole subtree (see
    /// [`user_data_subtree_roots`]).
    protected_subtree: Vec<PathBuf>,
}

impl Guard {
    /// Build a guard from a user keep list.
    ///
    /// The protected-root set is deliberately **not** configurable and has no
    /// override. It is the last line of defence between a cleaner (or an
    /// imported third-party definition) and an immutable system root, so it has
    /// to fail closed: `--force` only bypasses the running-application refusal,
    /// never this.
    pub fn new(keep: KeepList) -> Self {
        Self {
            keep,
            protected: protected_roots(),
            protected_subtree: user_data_subtree_roots(),
        }
    }

    /// Guard with an empty user keep list.
    pub fn default_guard() -> Self {
        Self::new(KeepList::new())
    }

    /// Merge the built-in conventions into the user keep list.
    pub fn with_builtins(mut self) -> Self {
        let builtins = builtin_keep_list();
        self.keep.files.extend(builtins.files().cloned());
        self.keep.folders.extend(builtins.folders().cloned());
        self
    }

    /// The immutable system roots (node-protected).
    pub fn protected(&self) -> &[PathBuf] {
        &self.protected
    }

    /// User-data directories protected as a whole subtree.
    pub fn protected_subtree(&self) -> &[PathBuf] {
        &self.protected_subtree
    }

    /// The user-configurable keep list.
    pub fn keep(&self) -> &KeepList {
        &self.keep
    }

    /// Is this path (or an ancestor) on the user keep list?
    pub fn is_kept(&self, path: &Path) -> bool {
        self.keep.contains(path)
    }

    /// Is this path one of the immutable system roots, or inside a user-data
    /// subtree that must be kept whole?
    pub fn is_protected(&self, path: &Path) -> bool {
        self.is_protected_normalized(&crate::core::path::normalize(path))
    }

    /// [`Guard::is_protected`] for an already normalised path.
    pub fn is_protected_normalized(&self, path: &Path) -> bool {
        if self.protected.iter().any(|root| path_equal(path, root)) {
            return true;
        }
        let case_sensitive = crate::core::path::case_sensitive();
        self.protected_subtree
            .iter()
            .any(|root| path_starts_with_cs(path, root, case_sensitive))
    }

    /// The single gate every destructive operation must pass.
    ///
    /// * [`Error::ProtectedPath`] — hard stop for immutable system roots.
    /// * [`Error::Link`] — the operation would follow a symlink / junction.
    /// * [`Error::Aborted`] — swallowed by the worker as "skip, not an error".
    pub fn check(&self, path: &Path, follow_links: bool) -> Result<()> {
        if !follow_links && is_link(path) {
            return Err(Error::Link(path.to_path_buf()));
        }
        // Normalise once and reuse it for both tests: this runs for every
        // candidate path of every cleaner, so the second allocation and
        // component walk were pure overhead on a large `--all` run.
        let normalized = crate::core::path::normalize(path);
        if self.is_protected_normalized(&normalized) {
            return Err(Error::ProtectedPath(path.to_path_buf()));
        }
        if self
            .keep
            .matches_normalized(&normalized, crate::core::path::case_sensitive())
        {
            return Err(Error::Aborted);
        }
        Ok(())
    }
}

/// True for symlinks on POSIX and reparse points (symlink/junction) on Windows.
pub fn is_link(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(md) => md.file_type().is_symlink() || platform::is_reparse_point(path),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_list_matches_children() {
        let mut list = KeepList::new();
        list.add_folder(PathBuf::from("/tmp/keepme"));
        assert!(list.contains(Path::new("/tmp/keepme/child.txt")));
        assert!(!list.contains(Path::new("/tmp/other")));
    }

    #[test]
    fn user_data_subtree_is_protected() {
        let g = Guard::default_guard();
        // System roots are node-protected (their contents are handled by the
        // deep safety layer with an allowlist).
        if !cfg!(windows) {
            assert!(g.is_protected(Path::new("/")));
            assert!(g.is_protected(Path::new("/etc")));
        }
        // User-data directories protect the node AND everything beneath it.
        if let Some(home) = crate::platform::home_dir() {
            let docs = home.join("Documents");
            assert!(g.is_protected(&docs));
            assert!(g.is_protected(&docs.join("thesis.pdf")));
            let ssh = home.join(".ssh");
            assert!(g.is_protected(&ssh));
            assert!(g.is_protected(&ssh.join("id_rsa")));
            // Cleaner targets stay node-only: children remain deletable.
            let cfg = home.join(".config");
            assert!(g.is_protected(&cfg));
            assert!(!g.is_protected(&cfg.join("chromium")));
        }
    }

    /// Regression guard for a real footgun: the protected-root set used to be
    /// disabled by `--force` (the CLI passed `cli.force` into `Guard::new`), so
    /// `sweep clean --force` could delete `/etc` or `C:\Windows`. There is no
    /// override any more — `check()` must always refuse an immutable root.
    #[test]
    fn protected_roots_are_never_overridable() {
        let g = Guard::default_guard();
        #[cfg(unix)]
        let root = PathBuf::from("/etc");
        #[cfg(windows)]
        let root =
            PathBuf::from(std::env::var("windir").unwrap_or_else(|_| "C:\\Windows".to_string()));

        assert!(
            g.is_protected(&root),
            "{} must stay in the protected set",
            root.display()
        );
        match g.check(&root, false) {
            Err(Error::ProtectedPath(_)) => {}
            other => panic!(
                "expected ProtectedPath for {}, got {other:?}",
                root.display()
            ),
        }
    }
}
