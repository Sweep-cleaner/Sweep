//! Progress reporting for preview/clean runs.
//!
//! The worker emits entries through a [`Progress`] sink rather than pushing
//! them to the terminal directly. That keeps the engine testable and lets the
//! CLI swap in a `ProgressBar` for a TTY and a no-op for scripting.

use std::sync::Arc;

use crate::core::report::Entry;

/// Anything that wants to observe a run as it happens.
pub trait Progress: Send + Sync {
    /// Called for every path the engine considers acting on.
    fn on_candidate(&self, path: &std::path::Path, would_delete: bool);

    /// Called once a concrete operation has been recorded.
    fn on_entry(&self, entry: &Entry);

    /// Free-form status line (e.g. "Scanning ~/.cache/firefox").
    fn set_message(&self, message: &str);

    /// `true` when the user has asked to abort.
    fn is_cancelled(&self) -> bool;

    /// Total number of operations recorded so far (for the progress bar).
    fn count(&self) -> u64;
}

/// Progress that records nothing and never cancels.
pub struct NoProgress {
    n: std::sync::atomic::AtomicU64,
}

impl Default for NoProgress {
    fn default() -> Self {
        Self::new()
    }
}

impl NoProgress {
    /// New no-op progress tracker.
    pub fn new() -> Self {
        Self {
            n: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

impl Progress for NoProgress {
    fn on_candidate(&self, _path: &std::path::Path, _would_delete: bool) {}
    fn on_entry(&self, _entry: &Entry) {
        self.n.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn set_message(&self, _message: &str) {}
    fn is_cancelled(&self) -> bool {
        false
    }
    fn count(&self) -> u64 {
        self.n.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// A `ProgressBar`-backed progress sink for interactive use.
pub struct CliProgress {
    bar: std::sync::Mutex<indicatif::ProgressBar>,
}

impl CliProgress {
    /// Build from an existing bar.
    pub fn new(bar: indicatif::ProgressBar) -> Arc<Self> {
        Arc::new(Self {
            bar: std::sync::Mutex::new(bar),
        })
    }
}

impl Progress for CliProgress {
    fn on_candidate(&self, _path: &std::path::Path, _would_delete: bool) {}
    fn on_entry(&self, entry: &Entry) {
        if let Ok(bar) = self.bar.lock() {
            bar.inc(1);
            if let Some(path) = &entry.path {
                bar.set_message(format!("{} {}", entry.label, path.display()));
            } else {
                bar.set_message(entry.label.clone());
            }
        }
    }
    fn set_message(&self, message: &str) {
        if let Ok(bar) = self.bar.lock() {
            bar.set_message(message.to_string());
        }
    }
    fn is_cancelled(&self) -> bool {
        false
    }
    fn count(&self) -> u64 {
        self.bar.lock().map(|bar| bar.position()).unwrap_or(0)
    }
}

/// Type-erased progress handle shared across threads.
pub type SharedProgress = Arc<dyn Progress>;
