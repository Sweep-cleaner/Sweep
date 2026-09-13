//! Directory traversal and glob resolution.
//!
//! Design constraints inherited from BleachBit, plus one upgrade:
//!
//! * never descend into symlinks or junctions (a cleaner must not escape its
//!   declared tree);
//! * emit directories *after* their contents so a `walk.all` delete removes
//!   children before trying to `rmdir` the parent;
//! * a permission error on one subdirectory must not abort the whole scan;
//! * **new**: results can be produced in parallel via [`scan_paths_parallel`],
//!   which is where most of Sweep's speed advantage over the Python
//!   implementation comes from.

use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobMatcher};
use walkdir::WalkDir;

use crate::core::error::{Error, Result};
use crate::platform;

/// Options for a directory scan.
#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    /// Also yield directories (before they are descended into, but after their
    /// contents, i.e. `contents_first` ordering).
    pub include_dirs: bool,
    /// Follow symlinks — almost always `false`; used only by the rare cleaner
    /// that explicitly asks for it.
    pub follow_links: bool,
    /// Maximum recursion depth (`None` = unlimited).
    pub max_depth: Option<usize>,
    /// Stay on the same filesystem as the root (uses `st_dev`).
    pub same_filesystem: bool,
    /// Yield the root itself instead of only its children.
    pub include_root: bool,
}

impl ScanOptions {
    /// Scan files only, unlimited depth, no link following.
    pub fn files() -> Self {
        Self::default()
    }

    /// Scan files and directories, directories last.
    pub fn all() -> Self {
        Self {
            include_dirs: true,
            ..Self::default()
        }
    }

    /// Builder-style depth limit.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Builder-style: stay on the filesystem that contains the root.
    ///
    /// The deletion path enables this. A cleaner target such as `~/.cache` or
    /// `/tmp` must not let the walk descend into a *different* volume that
    /// happens to be mounted underneath it — otherwise a USB stick or network
    /// share mounted at `~/.cache/usb` would be emptied along with the cache.
    pub fn with_same_filesystem(mut self, yes: bool) -> Self {
        self.same_filesystem = yes;
        self
    }
}

/// Does the pattern actually contain a wildcard?
///
/// A cleaner that declares `search="glob"` but writes a literal path is almost
/// certainly a bug; BleachBit only logs it, we also fall back to a plain
/// existence check, which is the behaviour the author intended.
pub fn has_glob(pattern: &str) -> bool {
    pattern.contains(['*', '?', '[', ']', '{'])
}

fn build_matcher(pattern: &str) -> Result<GlobMatcher> {
    let unix = pattern.replace('\\', "/");
    let glob = GlobBuilder::new(&unix)
        // Match BleachBit's `glob.iglob()` semantics: `*` never crosses a
        // directory separator, `**` does.
        .literal_separator(true)
        .case_insensitive(cfg!(windows))
        .backslash_escape(true)
        .build()
        .map_err(|source| Error::Glob {
            pattern: pattern.to_string(),
            source,
        })?;
    Ok(glob.compile_matcher())
}

/// Split `/a/b/**/c/*.log` into the literal prefix `/a/b` and the remainder.
///
/// Walking from the literal prefix (instead of from `/`) turns an
/// O(filesystem) glob into an O(subtree) glob.
fn split_glob_root(pattern: &str) -> (PathBuf, bool) {
    let unix = pattern.replace('\\', "/");
    let mut literal: Vec<String> = Vec::new();
    let mut wildcard_seen = false;

    for part in unix.split('/') {
        if part.is_empty() {
            continue;
        }
        if !wildcard_seen && has_glob(part) {
            wildcard_seen = true;
        }
        if wildcard_seen {
            break;
        }
        literal.push(part.to_string());
    }

    if literal.is_empty() {
        // Pattern starts with a wildcard: walk from "." and let the matcher
        // decide. Absolute patterns always carry a literal prefix, so this only
        // happens for bare `*.log` / `sub/*` style patterns.
        return (PathBuf::from("."), wildcard_seen);
    }

    // Reconstruct the literal prefix. A Windows drive letter must be preserved
    // as an *absolute* root: naively pushing `C:` as a plain component yields a
    // drive-relative path (`C:Users\foo`) that resolves against the process CWD
    // on that drive, so a cleaner glob would silently walk the wrong directory.
    // Re-attaching the separator makes it absolute (`C:\Users\foo`).
    let mut root = if literal[0].len() == 2
        && literal[0].as_bytes()[1] == b':'
        && literal[0].as_bytes()[0].is_ascii_alphabetic()
    {
        let mut base = literal.remove(0);
        base.push(std::path::MAIN_SEPARATOR);
        PathBuf::from(base)
    } else if unix.starts_with("//") {
        PathBuf::from("//")
    } else if unix.starts_with('/') {
        PathBuf::from(std::path::MAIN_SEPARATOR.to_string())
    } else {
        PathBuf::new()
    };

    for part in literal {
        root.push(part);
    }

    (root, wildcard_seen)
}

/// Expand a glob pattern into matching paths.
///
/// Directories are returned after their contents, exactly like [`scan_paths`].
pub fn glob_paths(pattern: &str) -> Result<Vec<PathBuf>> {
    if !has_glob(pattern) {
        let path = PathBuf::from(pattern);
        return Ok(if path.exists() || path.symlink_metadata().is_ok() {
            vec![path]
        } else {
            Vec::new()
        });
    }

    let matcher = build_matcher(pattern)?;
    let (root, _) = split_glob_root(pattern);
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    let mut dirs = Vec::new();

    // `glob_paths` only ever serves cleaner definitions, so it stays on the
    // root's filesystem for the same reason `scan_paths` does: a glob such as
    // `~/.cache/**/*.log` must not reach into a volume mounted underneath it.
    let walker = WalkDir::new(&root)
        .follow_links(false)
        .contents_first(true)
        .same_file_system(true);
    let root_dev = platform::device_id(&root);

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        let candidate = entry.path();
        if let (Some(want), Some(got)) = (root_dev, platform::device_id(candidate)) {
            if want != got {
                continue;
            }
        }
        let unix = candidate.to_string_lossy().replace('\\', "/");
        if !matcher.is_match(&unix) {
            continue;
        }
        if entry.file_type().is_dir() {
            dirs.push(candidate.to_path_buf());
        } else {
            files.push(candidate.to_path_buf());
        }
    }

    files.extend(dirs);
    Ok(files)
}

/// Like [`glob_paths`] but keeps only directories (used by `<var search="glob">`).
pub fn glob_dirs(pattern: &str) -> Result<Vec<PathBuf>> {
    Ok(glob_paths(pattern)?
        .into_iter()
        .filter(|p| p.is_dir())
        .collect())
}

/// Everything inside `root` (not `root` itself).
///
/// This is the direct equivalent of BleachBit's `children_in_directory()`.
pub fn children(root: &Path, include_dirs: bool) -> Vec<PathBuf> {
    let options = ScanOptions {
        include_dirs,
        ..ScanOptions::default()
    };
    scan_paths(root, &options)
}

/// Full scan of `root` according to `options`.
pub fn scan_paths(root: &Path, options: &ScanOptions) -> Vec<PathBuf> {
    if root.symlink_metadata().is_err() {
        return Vec::new();
    }

    let mut walker = WalkDir::new(root)
        .follow_links(options.follow_links)
        .contents_first(true)
        .min_depth(if options.include_root { 0 } else { 1 })
        .same_file_system(options.same_filesystem);

    if let Some(depth) = options.max_depth {
        walker = walker.max_depth(depth);
    }

    let root_dev = if options.same_filesystem {
        platform::device_id(root)
    } else {
        None
    };

    let mut files = Vec::new();
    let mut dirs = Vec::new();

    for entry in walker.into_iter().filter_entry(|e| {
        // Never descend into a link unless explicitly asked to. WalkDir's
        // follow_links(false) already handles POSIX symlinks; junctions on
        // Windows need the explicit reparse-point check.
        if e.depth() == 0 {
            return true;
        }
        options.follow_links || !platform::is_reparse_point(e.path())
    }) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                // A single unreadable directory must not abort the scan.
                log::debug!("skipping unreadable entry: {err}");
                continue;
            }
        };

        if let (Some(want), Some(got)) = (root_dev, platform::device_id(entry.path())) {
            if want != got {
                continue;
            }
        }

        let is_dir = entry.file_type().is_dir();
        if is_dir {
            if options.include_dirs {
                dirs.push(entry.into_path());
            }
        } else {
            files.push(entry.into_path());
        }
    }

    files.extend(dirs);
    files
}

/// Parallel variant: the scan itself stays single threaded (directory ordering
/// matters) but the per-entry work in `f` runs on the rayon pool.
///
/// Returns the collected results in scan order.
pub fn scan_paths_parallel<T, F>(root: &Path, options: &ScanOptions, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(&Path) -> T + Send + Sync,
{
    use rayon::prelude::*;
    let paths = scan_paths(root, options);
    paths.par_iter().map(|p| f(p.as_path())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute glob must walk the directory exactly as written. On Windows a
    /// drive-letter pattern (`C:\Users\…\*.log`) used to be rebuilt as a
    /// drive-relative path (`C:Users\…`) that resolved against the process CWD
    /// on that drive, so the glob silently matched nothing. This is the
    /// regression guard for that bug.
    #[test]
    fn glob_preserves_absolute_root() {
        let base = std::env::temp_dir().join("sweep_glob_root_test");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("sub")).unwrap();
        std::fs::write(base.join("sub").join("a.log"), b"x").unwrap();

        let pattern = base.join("sub").join("*.log");
        let pattern = pattern.to_string_lossy().into_owned();
        let hits = glob_paths(&pattern).expect("glob should not error");
        assert!(
            hits.iter().any(|p| p.ends_with("a.log")),
            "absolute glob '{pattern}' matched nothing: {hits:?}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn has_glob_detects_wildcards() {
        assert!(has_glob("a*.log"));
        assert!(has_glob("a?b"));
        assert!(has_glob("a[0-9]"));
        assert!(has_glob("a{b,c}"));
        assert!(!has_glob("a/b/c.log"));
        assert!(!has_glob(""));
    }

    #[test]
    fn scan_never_follows_links() {
        let base = std::env::temp_dir().join("sweep_scan_link_test");
        let _ = std::fs::remove_dir_all(&base);
        let src = base.join("real");
        let _ = std::fs::remove_dir_all(&src);
        std::fs::create_dir_all(src.join("inner")).unwrap();
        std::fs::write(src.join("inner").join("f.txt"), b"data").unwrap();

        let outside = std::env::temp_dir().join("sweep_scan_link_outside");
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), b"secret").unwrap();

        // A symlink whose target lives *outside* the scanned root.
        let link = base.join("link");
        // Creating a symlink needs elevation on Windows; if we cannot, the test
        // proves nothing about escaping, so skip rather than fail.
        let made = {
            let _ = std::fs::remove_dir_all(&link);
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&outside, &link).is_ok()
            }
            #[cfg(windows)]
            {
                std::os::windows::fs::symlink_dir(&outside, &link).is_ok()
            }
        };
        if !made {
            let _ = std::fs::remove_dir_all(&base);
            let _ = std::fs::remove_dir_all(&outside);
            return;
        }

        let found: Vec<PathBuf> = scan_paths(&base, &ScanOptions::all());
        assert!(
            !found.iter().any(|p| p.ends_with("secret.txt")),
            "walk escaped the declared root through a symlink: {found:?}"
        );

        let _ = std::fs::remove_dir_all(&base);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// The deletion path enables `same_filesystem`, so it must not exclude
    /// children that *are* on the root's own filesystem. If `device_id` were
    /// wrong (or returned `None` inconsistently) every cleaner walk would
    /// silently match nothing — a failure mode that looks exactly like "nothing
    /// to clean".
    #[test]
    fn same_filesystem_keeps_children_on_the_root_device() {
        let base = std::env::temp_dir().join("sweep_samefs_test");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("sub")).unwrap();
        std::fs::write(base.join("sub").join("a.txt"), b"x").unwrap();
        std::fs::write(base.join("b.txt"), b"y").unwrap();

        let found = scan_paths(&base, &ScanOptions::files().with_same_filesystem(true));
        assert_eq!(found.len(), 2, "same-filesystem scan lost files: {found:?}");

        let pattern = format!("{}/**/*.txt", base.to_string_lossy());
        let globbed = glob_paths(&pattern).expect("glob should not error");
        assert!(
            globbed.iter().any(|p| p.ends_with("a.txt")),
            "same-filesystem glob lost sub/a.txt: {globbed:?}"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
