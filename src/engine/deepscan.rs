//! Deep scan.
//!
//! BleachBit's most-requested feature and its biggest performance sink: given
//! a set of root directories, descend into them and flag every file that
//! matches a user-supplied pattern (`*.log`, `*/cache/*`, `*.tmp`, ...).
//! Sweep runs the per-root scan in parallel and compiles the patterns once.
//!
//! Deep scan is offered as its own command (`sweep deepscan`) and is *also*
//! reachable from any cleaner option that declares `<action search="deep">`.

use std::path::Path;
use std::sync::Arc;

use globset::{GlobBuilder, GlobMatcher};
use rayon::prelude::*;

use crate::action::context::RunContext;
use crate::core::error::Result;
use crate::core::report::{Entry, EntryKind, Report};
use crate::fsutil::size::size_of;
use crate::fsutil::walk::{scan_paths, ScanOptions};

/// One deep-scan rule: a glob matched against the whole path.
#[derive(Debug, Clone)]
pub struct DeepRule {
    /// Original glob, for diagnostics.
    pub pattern: String,
    /// Compiled matcher.
    matcher: GlobMatcher,
}

impl DeepRule {
    /// Compile `pattern`.
    pub fn new(pattern: &str) -> Result<Self> {
        let matcher = GlobBuilder::new(pattern)
            .case_insensitive(true)
            .literal_separator(false)
            .build()
            .map_err(|source| crate::core::error::Error::Glob {
                pattern: pattern.to_string(),
                source,
            })?
            .compile_matcher();
        Ok(Self {
            pattern: pattern.to_string(),
            matcher,
        })
    }

    /// Does `path` match?
    pub fn matches(&self, path: &Path) -> bool {
        self.matcher.is_match(path)
    }
}

/// Compile a list of deep-scan patterns (invalid ones are skipped with a warning).
pub fn compile_rules(patterns: &[String]) -> Vec<DeepRule> {
    patterns
        .iter()
        .filter_map(|p| match DeepRule::new(p) {
            Ok(rule) => Some(rule),
            Err(err) => {
                log::warn!("ignoring invalid deep-scan pattern '{p}': {err}");
                None
            }
        })
        .collect()
}

/// Run a deep scan over `roots`, recording every match.
pub fn scan(roots: &[std::path::PathBuf], rules: &[DeepRule], ctx: &RunContext) -> Report {
    let mut report = Report::new();
    let rules = Arc::new(rules.to_vec());

    let hits: Vec<(std::path::PathBuf, u64)> = roots
        .par_iter()
        .flat_map(|root| {
            if !root.is_dir() {
                return Vec::new();
            }
            let options = ScanOptions::files().with_max_depth(32);
            scan_paths(root, &options)
                .into_iter()
                .filter(|path| rules.iter().any(|rule| rule.matches(path)))
                .map(|path| {
                    let size = size_of(&path).unwrap_or(0);
                    (path, size)
                })
                .collect::<Vec<_>>()
        })
        .collect();

    for (path, size) in hits {
        if ctx.cancelled() {
            report.aborted = true;
            break;
        }
        if ctx.dry_run {
            report.push(Entry::new(
                EntryKind::Delete,
                "deepscan",
                "deepscan",
                format!("match {}", path.display()),
                Some(path.as_path()),
                size,
            ));
        } else {
            crate::deep::guarded_delete(&path, ctx, "deepscan", "deepscan", &mut report);
        }
    }

    report
}

/// The default deep-scan patterns BleachBit ships with, expressed for globset.
pub fn default_patterns() -> Vec<String> {
    vec![
        "*.log".to_string(),
        "*.log.*".to_string(),
        "*.tmp".to_string(),
        "*.temp".to_string(),
        "*.bak".to_string(),
        "*.old".to_string(),
        "*.~*".to_string(),
        "*~".to_string(),
        "*.swp".to_string(),
        "*.swo".to_string(),
        "*.orig".to_string(),
        "*.rej".to_string(),
        ".nfs*".to_string(),
        "*cache*".to_string(),
        "Thumbs.db".to_string(),
        ".DS_Store".to_string(),
    ]
}
