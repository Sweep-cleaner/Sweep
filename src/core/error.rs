//! Unified error type for every fallible operation in Sweep.
//!
//! BleachBit lets Python exceptions propagate through the worker and formats
//! them at the very end. Rust forces us to be explicit instead, so we keep a
//! single [`Error`] enum that is (a) cheap to match on and (b) always carries
//! enough context to print a useful one-line message without a backtrace.

use std::path::{Path, PathBuf};

use thiserror::Error;

/// Anything that can go wrong while previewing or cleaning.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O failure tied to a concrete path.
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// An I/O failure with no meaningful path attached.
    #[error("i/o error: {0}")]
    PlainIo(#[from] std::io::Error),

    /// A cleaner tried to touch a path that is always off limits (`/`, `C:\Windows`, ...).
    #[error("refusing to operate on protected path: {0}")]
    ProtectedPath(PathBuf),

    /// A symlink / junction / reparse point was found where a real file was required.
    #[error("refusing to follow a link: {0}")]
    Link(PathBuf),

    /// A cleaner definition (TOML, CleanerML or winapp2.ini) is malformed.
    #[error("invalid cleaner definition in {file}: {reason}")]
    Definition { file: String, reason: String },

    /// The definition references an action command that does not exist.
    #[error("cleaner '{cleaner}' uses unknown action command '{command}'")]
    UnknownAction { cleaner: String, command: String },

    /// A requested `cleaner.option` pair does not exist.
    #[error("unknown option '{option}' for cleaner '{cleaner}'")]
    UnknownOption { cleaner: String, option: String },

    /// A cleaner id was requested that is not registered.
    #[error("unknown cleaner '{0}'")]
    UnknownCleaner(String),

    /// A glob pattern failed to compile.
    #[error("invalid glob pattern '{pattern}': {source}")]
    Glob {
        pattern: String,
        #[source]
        source: globset::Error,
    },

    /// A regular expression failed to compile.
    #[error("invalid regular expression '{pattern}': {source}")]
    Regex {
        pattern: String,
        #[source]
        source: regex::Error,
    },

    /// SQLite failure.
    #[error("database error on {path}: {source}")]
    Database {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    /// INI / JSON / XML editing failure.
    #[error("failed to edit structured file {path}: {reason}")]
    Structured { path: PathBuf, reason: String },

    /// Configuration file could not be read or written.
    #[error("configuration error: {0}")]
    Config(String),

    /// An external helper (`apt-get clean`, `dnf clean all`, ...) failed.
    #[error("external command '{command}' failed with status {status}: {stderr}")]
    ExternalCommand {
        command: String,
        status: i32,
        stderr: String,
    },

    /// An external helper could not be started at all.
    #[error("cannot run '{command}': {reason}")]
    Spawn { command: String, reason: String },

    /// The operation was cancelled by the user (Ctrl-C or the abort flag).
    #[error("operation aborted")]
    Aborted,

    /// Anything else, already formatted for humans.
    #[error("{0}")]
    Message(String),
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Wrap an [`std::io::Error`] with the path that triggered it.
    pub fn io<P: AsRef<Path>>(path: P, source: std::io::Error) -> Self {
        Error::Io {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    /// Build an [`Error::Definition`].
    pub fn definition<S: Into<String>, R: Into<String>>(file: S, reason: R) -> Self {
        Error::Definition {
            file: file.into(),
            reason: reason.into(),
        }
    }

    /// Build an [`Error::Message`].
    pub fn msg<S: Into<String>>(s: S) -> Self {
        Error::Message(s.into())
    }

    /// True for errors that are expected during a normal system scan and must
    /// not abort the whole run (missing files, permission denied, locked DBs...).
    pub fn is_benign(&self) -> bool {
        match self {
            Error::Io { source, .. } | Error::PlainIo(source) => {
                matches!(
                    source.kind(),
                    std::io::ErrorKind::NotFound
                        | std::io::ErrorKind::PermissionDenied
                        | std::io::ErrorKind::AlreadyExists
                )
            }
            Error::Database { source, .. } => matches!(
                source,
                rusqlite::Error::SqliteFailure(_, _) | rusqlite::Error::QueryReturnedNoRows
            ),
            Error::ProtectedPath(_) | Error::Link(_) | Error::Aborted => true,
            _ => false,
        }
    }
}

impl From<globset::Error> for Error {
    fn from(source: globset::Error) -> Self {
        Error::Glob {
            pattern: String::new(),
            source,
        }
    }
}

impl From<regex::Error> for Error {
    fn from(source: regex::Error) -> Self {
        Error::Regex {
            pattern: String::new(),
            source,
        }
    }
}

/// Attach a path to a glob error, matching BleachBit's per-path diagnostics.
pub trait WithPath {
    /// Annotate the error with `path`.
    fn with_path<P: AsRef<Path>>(self, path: P) -> Error;
}

impl WithPath for globset::Error {
    fn with_path<P: AsRef<Path>>(self, path: P) -> Error {
        Error::Glob {
            pattern: path.as_ref().display().to_string(),
            source: self,
        }
    }
}

impl WithPath for regex::Error {
    fn with_path<P: AsRef<Path>>(self, path: P) -> Error {
        Error::Regex {
            pattern: path.as_ref().display().to_string(),
            source: self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn io_error(kind: std::io::ErrorKind) -> std::io::Error {
        std::io::Error::new(kind, "boom")
    }

    #[test]
    fn constructors_carry_their_context() {
        let err = Error::io("/tmp/a b", io_error(std::io::ErrorKind::PermissionDenied));
        assert!(matches!(&err, Error::Io { path, .. } if path == Path::new("/tmp/a b")));

        let err = Error::definition("apt.toml", "missing id");
        assert!(matches!(&err, Error::Definition { file, reason }
            if file == "apt.toml" && reason == "missing id"));

        let err = Error::msg("nope");
        assert!(matches!(err, Error::Message(text) if text == "nope"));
    }

    #[test]
    fn messages_are_single_line_and_useful() {
        let cases: Vec<(Error, &str)> = vec![
            (
                Error::ProtectedPath(PathBuf::from("/")),
                "refusing to operate on protected path: /",
            ),
            (
                Error::UnknownOption {
                    cleaner: "apt".into(),
                    option: "nope".into(),
                },
                "unknown option 'nope' for cleaner 'apt'",
            ),
            (Error::UnknownCleaner("apt".into()), "unknown cleaner 'apt'"),
            (
                Error::UnknownAction {
                    cleaner: "apt".into(),
                    command: "frobnicate".into(),
                },
                "cleaner 'apt' uses unknown action command 'frobnicate'",
            ),
            (Error::Aborted, "operation aborted"),
            (
                Error::Config("bad toml".into()),
                "configuration error: bad toml",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn benign_errors_do_not_abort_a_scan() {
        // Expected during a normal scan: the file vanished, we may not read it,
        // or a guard refused it. None of these should abort the whole run.
        for kind in [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::AlreadyExists,
        ] {
            assert!(Error::io("/x", io_error(kind)).is_benign(), "{kind:?}");
        }
        assert!(Error::from(io_error(std::io::ErrorKind::NotFound)).is_benign());
        assert!(Error::ProtectedPath(PathBuf::from("/etc")).is_benign());
        assert!(Error::Link(PathBuf::from("/x")).is_benign());
        assert!(Error::Aborted.is_benign());
        assert!(Error::Database {
            path: PathBuf::from("/x.sqlite"),
            source: rusqlite::Error::QueryReturnedNoRows,
        }
        .is_benign());
    }

    #[test]
    fn real_failures_are_not_benign() {
        assert!(!Error::io("/x", io_error(std::io::ErrorKind::Other)).is_benign());
        assert!(!Error::msg("definition is broken").is_benign());
        assert!(!Error::UnknownCleaner("apt".into()).is_benign());
        assert!(!Error::Spawn {
            command: "apt-get".into(),
            reason: "not found".into(),
        }
        .is_benign());
    }

    /// The first pattern the underlying crate refuses; keeps the test honest
    /// without hard-coding which syntax each version rejects.
    fn bad_glob() -> globset::Error {
        ["[", "{a", "**a", "a\\"]
            .into_iter()
            .find_map(|p| globset::Glob::new(p).err())
            .expect("at least one of these globs is invalid")
    }

    fn bad_regex() -> regex::Error {
        ["(", "[", "*", "a{2,1}"]
            .into_iter()
            .find_map(|p| regex::Regex::new(p).err())
            .expect("at least one of these regexes is invalid")
    }

    #[test]
    fn bad_patterns_can_be_annotated_with_a_path() {
        // A bare `?` has no context, so the path is attached afterwards.
        let converted: Error = bad_glob().into();
        assert!(matches!(converted, Error::Glob { ref pattern, .. } if pattern.is_empty()));

        let annotated = bad_glob().with_path("/tmp/x");
        assert!(
            matches!(annotated, Error::Glob { ref pattern, .. } if pattern == "/tmp/x"),
            "{annotated:?}"
        );
        assert!(annotated.to_string().contains("/tmp/x"));

        let converted: Error = bad_regex().into();
        assert!(matches!(converted, Error::Regex { ref pattern, .. } if pattern.is_empty()));
        assert!(matches!(bad_regex().with_path("/tmp/y"),
                     Error::Regex { ref pattern, .. } if pattern == "/tmp/y"));
    }
}
