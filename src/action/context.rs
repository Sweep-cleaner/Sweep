//! Execution context shared by every action.
//!
//! Kept deliberately small and cheap to clone (the worker hands a reference to
//! each parallel option). Actions read from it; they never mutate it.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::core::keep::Guard;
use crate::engine::progress::{NoProgress, SharedProgress};
use crate::shred::ShredSpec;

/// Everything an action needs to decide *whether* and *how* to destroy data.
#[derive(Clone)]
pub struct RunContext {
    /// The keep/protection gate. Every destructive step passes through it.
    pub guard: Guard,
    /// Preview only — never touch the filesystem.
    pub dry_run: bool,
    /// `--shred` on the command line: overwrite everything before deleting.
    pub global_shred: bool,
    /// Pass count / pattern for the secure-delete path.
    pub shred_spec: ShredSpec,
    /// Abort switch, flipped by Ctrl-C.
    pub cancel: Arc<AtomicBool>,
    /// Live progress sink.
    pub progress: SharedProgress,
    /// Locales to preserve when `system.localizations` runs.
    pub keep_localizations: Vec<String>,
    /// User-configured custom paths (`system.custom`).
    pub custom_paths: Vec<PathBuf>,
    /// Roots for the deep-scan phase.
    pub deepscan_roots: Vec<PathBuf>,
    /// Optional backup root: files are copied here (mirroring layout) before deletion.
    pub backup_dir: Option<PathBuf>,
    /// Operation log file. `None` selects the platform default.
    pub log_file: Option<PathBuf>,
    /// User-tunable deep-clean budgets, from the config file.
    pub deep_options: crate::deep::DeepOptions,
    /// Interface language for engine report strings.
    pub lang: crate::i18n::Lang,
}

impl Default for RunContext {
    fn default() -> Self {
        Self {
            guard: Guard::default_guard().with_builtins(),
            dry_run: true,
            global_shred: false,
            shred_spec: ShredSpec::default_spec(),
            cancel: Arc::new(AtomicBool::new(false)),
            progress: Arc::new(NoProgress::new()),
            keep_localizations: Vec::new(),
            custom_paths: Vec::new(),
            deepscan_roots: Vec::new(),
            backup_dir: None,
            log_file: None,
            deep_options: crate::deep::DeepOptions::default(),
            lang: crate::i18n::Lang::new("en"),
        }
    }
}

impl RunContext {
    /// Preview (dry-run) context with sane defaults.
    pub fn preview() -> Self {
        Self::default()
    }

    /// A real cleaning context (mutable filesystem).
    pub fn clean() -> Self {
        Self {
            dry_run: false,
            ..Self::default()
        }
    }

    /// Should this run abort before the next unit of work?
    pub fn cancelled(&self) -> bool {
        self.cancel.load(std::sync::atomic::Ordering::Relaxed)
    }
}
