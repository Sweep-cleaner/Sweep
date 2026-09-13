//! Accounting: what happened, how much was reclaimed, what failed.
//!
//! BleachBit's `Worker` returns a dict per command and mutates counters on the
//! UI object. Sweep instead accumulates an append-only [`Report`], which
//! is trivially safe to build from a `rayon` parallel iterator and can be
//! serialised to JSON for scripting.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// What a single command did (or would do).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// The path was unlinked.
    Delete,
    /// The path was overwritten before unlinking.
    Shred,
    /// The path was truncated to zero length but kept.
    Truncate,
    /// A structured file (INI/JSON/XML/SQLite) was edited in place.
    CleanFile,
    /// `VACUUM` was run on a SQLite database.
    Vacuum,
    /// A registry key or value was removed (Windows).
    Registry,
    /// An external helper ran (`apt-get clean`, ...).
    Command,
    /// Free space was overwritten.
    WipeFreeSpace,
    /// The path matched the keep list, so nothing happened.
    Skip,
    /// Something failed; see [`Report::errors`].
    Error,
}

impl EntryKind {
    /// Short human label, mirroring BleachBit's log column.
    pub fn label(self) -> &'static str {
        match self {
            EntryKind::Delete => "Delete",
            EntryKind::Shred => "Shred",
            EntryKind::Truncate => "Truncate",
            EntryKind::CleanFile => "Clean file",
            EntryKind::Vacuum => "Vacuum",
            EntryKind::Registry => "Delete registry key",
            EntryKind::Command => "Run command",
            EntryKind::WipeFreeSpace => "Wipe free space",
            EntryKind::Skip => "Skip",
            EntryKind::Error => "Error",
        }
    }

    /// Does this entry represent one fewer file on disk?
    pub fn counts_as_deleted(self) -> bool {
        matches!(self, EntryKind::Delete | EntryKind::Shred)
    }

    /// Does this entry represent a "special" operation (BleachBit terminology)?
    pub fn counts_as_special(self) -> bool {
        matches!(
            self,
            EntryKind::CleanFile
                | EntryKind::Vacuum
                | EntryKind::Registry
                | EntryKind::Command
                | EntryKind::WipeFreeSpace
        )
    }
}

/// One line of the cleaning log.
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    /// What was done.
    pub kind: EntryKind,
    /// Cleaner id, e.g. `firefox`.
    pub cleaner: String,
    /// Option id, e.g. `cache`.
    pub option: String,
    /// Human readable description shown in the log.
    pub label: String,
    /// Affected path, when the operation is path-based.
    pub path: Option<PathBuf>,
    /// Bytes that were (or would be) reclaimed.
    pub reclaimed: u64,
}

impl Entry {
    /// Build an entry.
    pub fn new(
        kind: EntryKind,
        cleaner: impl Into<String>,
        option: impl Into<String>,
        label: impl Into<String>,
        path: Option<&Path>,
        reclaimed: u64,
    ) -> Self {
        Self {
            kind,
            cleaner: cleaner.into(),
            option: option.into(),
            label: label.into(),
            path: path.map(|p| p.to_path_buf()),
            reclaimed,
        }
    }

    /// Entry for a path that was skipped because of the keep list.
    pub fn skipped(cleaner: &str, option: &str, path: &Path) -> Self {
        Self::new(
            EntryKind::Skip,
            cleaner,
            option,
            EntryKind::Skip.label(),
            Some(path),
            0,
        )
    }
}

/// Wall-clock stopwatch used to fill in [`Report::duration_ms`].
///
/// Kept here (next to the field it feeds) so every command measures its run
/// the same way instead of each one hand-rolling an `Instant`.
#[derive(Debug, Clone, Copy)]
pub struct Stopwatch(std::time::Instant);

impl Stopwatch {
    /// Start measuring now.
    pub fn start() -> Self {
        Self(std::time::Instant::now())
    }

    /// Milliseconds elapsed since [`Stopwatch::start`], saturating at `u64::MAX`.
    pub fn elapsed_ms(self) -> u64 {
        let ms = self.0.elapsed().as_millis();
        u64::try_from(ms).unwrap_or(u64::MAX)
    }
}

/// A non-fatal problem encountered during the run.
#[derive(Debug, Clone, Serialize)]
pub struct Failure {
    /// Cleaner id (may be empty for engine-level failures).
    pub cleaner: String,
    /// Option id.
    pub option: String,
    /// Description of the failure.
    pub message: String,
}

/// The full result of a preview or clean run.
#[derive(Debug, Default, Clone, Serialize)]
pub struct Report {
    /// Every recorded operation.
    pub entries: Vec<Entry>,
    /// Non-fatal failures; the run continued.
    pub errors: Vec<Failure>,
    /// Set when the user aborted before completion.
    pub aborted: bool,
    /// Wall-clock duration of the run in milliseconds.
    pub duration_ms: u64,
}

impl Report {
    /// Empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one entry.
    pub fn push(&mut self, entry: Entry) {
        self.entries.push(entry);
    }

    /// Drain another report into this one.
    pub fn merge(&mut self, other: Report) {
        self.entries.extend(other.entries);
        self.errors.extend(other.errors);
        self.aborted |= other.aborted;
        self.duration_ms = self.duration_ms.max(other.duration_ms);
    }

    /// Record a failure for `cleaner.option`.
    pub fn fail(&mut self, cleaner: &str, option: &str, message: impl Into<String>) {
        self.errors.push(Failure {
            cleaner: cleaner.to_string(),
            option: option.to_string(),
            message: message.into(),
        });
    }

    /// Total bytes reclaimed across all entries.
    pub fn reclaimed(&self) -> u64 {
        self.entries.iter().map(|e| e.reclaimed).sum()
    }

    /// Number of files removed.
    pub fn files_removed(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.kind.counts_as_deleted())
            .count() as u64
    }

    /// Number of "special" operations, matching BleachBit's statistic.
    pub fn special_operations(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.kind.counts_as_special())
            .count() as u64
    }

    /// Number of skipped paths.
    pub fn skipped(&self) -> u64 {
        self.entries
            .iter()
            .filter(|e| e.kind == EntryKind::Skip)
            .count() as u64
    }

    /// Was the run completely clean of errors?
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty() && !self.aborted
    }

    /// Stamp the wall-clock duration measured by `watch`.
    ///
    /// Keeps the longest value seen, so merging sub-reports that already
    /// carry their own timing never loses it.
    pub fn finish(&mut self, watch: Stopwatch) {
        self.duration_ms = self.duration_ms.max(watch.elapsed_ms());
    }

    /// Serialise to a pretty-printed JSON value (for `--json`).
    ///
    /// `bytes_affected` / `items_count` are the generic names the disk-outside
    /// commands (memopt, sysinfo, network, privacy) report under; they mirror
    /// `reclaimed_bytes` / `entries.len()` so a single consumer can read either
    /// vocabulary without special-casing the command.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "reclaimed_bytes": self.reclaimed(),
            "files_removed": self.files_removed(),
            "special_operations": self.special_operations(),
            "skipped": self.skipped(),
            "errors": self.errors.len(),
            "aborted": self.aborted,
            "duration_ms": self.duration_ms,
            "bytes_affected": self.reclaimed(),
            "items_count": self.entries.len(),
            "entries": self.entries,
            "failures": self.errors,
        })
    }

    /// Compact form of the report for a UI: exact totals, exact per-option
    /// aggregates, and only a bounded sample of the individual entries.
    ///
    /// `preview --all` is one entry per matched file, which on a developer
    /// machine is hundreds of thousands of entries and tens of megabytes of
    /// JSON — most of it about the same few cache trees. A GUI needs the bytes
    /// per `cleaner.option` and a handful of sample lines, not every path, so
    /// the full list is replaced by `by_option` and `entries` is sampled.
    /// Counters (`files_removed`, `reclaimed_bytes`, `items_count`, …) stay
    /// exact, so nothing that only reads totals is affected.
    pub fn to_json_summary(&self, max_entries: usize) -> serde_json::Value {
        use std::collections::BTreeMap;

        let mut grouped: BTreeMap<(&str, &str), (u64, u64)> = BTreeMap::new();
        for entry in &self.entries {
            let slot = grouped
                .entry((entry.cleaner.as_str(), entry.option.as_str()))
                .or_insert((0, 0));
            slot.0 += u64::from(entry.kind.counts_as_deleted());
            slot.1 += entry.reclaimed;
        }
        let by_option: Vec<serde_json::Value> = grouped
            .into_iter()
            .map(|((cleaner, option), (files, bytes))| {
                serde_json::json!({
                    "cleaner": cleaner,
                    "option": option,
                    "files": files,
                    "bytes": bytes,
                })
            })
            .collect();

        // Spread the sample over the whole run instead of taking the head:
        // the first entries are always the same cleaner alphabetically, which
        // would make every preview look identical.
        let sample: Vec<&Entry> = if max_entries == 0 || self.entries.len() <= max_entries {
            self.entries.iter().collect()
        } else {
            let step = self.entries.len() / max_entries;
            (0..max_entries)
                .map(|i| &self.entries[(i * step).min(self.entries.len() - 1)])
                .collect()
        };

        let truncated = sample.len() < self.entries.len();

        serde_json::json!({
            "reclaimed_bytes": self.reclaimed(),
            "files_removed": self.files_removed(),
            "special_operations": self.special_operations(),
            "skipped": self.skipped(),
            "errors": self.errors.len(),
            "aborted": self.aborted,
            "duration_ms": self.duration_ms,
            "bytes_affected": self.reclaimed(),
            "items_count": self.entries.len(),
            "entries_truncated": truncated,
            "by_option": by_option,
            "entries": sample,
            "failures": self.errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample() -> Report {
        let mut report = Report::new();
        report.push(Entry::new(
            EntryKind::Delete,
            "firefox",
            "cache",
            "cache entry",
            Some(Path::new("/home/u/.cache/f")),
            100,
        ));
        report.push(Entry::new(
            EntryKind::Shred,
            "firefox",
            "cache",
            "shredded",
            Some(Path::new("/home/u/.cache/g")),
            50,
        ));
        report.push(Entry::new(
            EntryKind::Vacuum,
            "firefox",
            "history",
            "db",
            Some(Path::new("/home/u/history.sqlite")),
            7,
        ));
        report.push(Entry::skipped("firefox", "cache", Path::new("/keep/me")));
        report
    }

    #[test]
    fn counters_split_by_kind() {
        let report = sample();
        // Delete + Shred count as removed; Vacuum is a special op; Skip is neither.
        assert_eq!(report.files_removed(), 2);
        assert_eq!(report.special_operations(), 1);
        assert_eq!(report.skipped(), 1);
        assert_eq!(report.reclaimed(), 157);
        assert!(report.is_ok());
    }

    #[test]
    fn kind_classification_is_stable() {
        assert!(EntryKind::Delete.counts_as_deleted());
        assert!(EntryKind::Shred.counts_as_deleted());
        assert!(!EntryKind::Truncate.counts_as_deleted());
        // A truncate frees bytes but removes nothing, so it is neither a
        // "file removed" nor a "special op" — it only shows up in `reclaimed`.
        assert!(!EntryKind::Truncate.counts_as_special());
        assert!(EntryKind::WipeFreeSpace.counts_as_special());
        assert!(EntryKind::CleanFile.counts_as_special());
        assert!(!EntryKind::Skip.counts_as_special());
        assert!(!EntryKind::Error.counts_as_special());
        // Every kind needs a human label — the CLI prints it verbatim.
        for kind in [
            EntryKind::Delete,
            EntryKind::Shred,
            EntryKind::Truncate,
            EntryKind::CleanFile,
            EntryKind::Vacuum,
            EntryKind::Registry,
            EntryKind::Command,
            EntryKind::WipeFreeSpace,
            EntryKind::Skip,
            EntryKind::Error,
        ] {
            assert!(!kind.label().is_empty());
        }
    }

    #[test]
    fn merge_keeps_worst_flags_and_longest_duration() {
        let mut left = sample();
        let mut right = Report::new();
        right.aborted = true;
        right.duration_ms = 500;
        right.fail("apt", "cache", "boom");
        left.merge(right);
        assert!(left.aborted);
        assert!(left.errors.iter().any(|f| f.option == "cache"));
        assert_eq!(left.duration_ms, 500);
        // A shorter later merge must not lower the recorded duration.
        let mut shorter = Report::new();
        shorter.duration_ms = 10;
        left.merge(shorter);
        assert_eq!(left.duration_ms, 500);
        // Entries accumulate.
        assert_eq!(left.entries.len(), 4);
        assert!(!left.is_ok());
    }

    #[test]
    fn finish_stamps_elapsed_time() {
        let mut report = Report::new();
        let watch = Stopwatch::start();
        std::thread::sleep(std::time::Duration::from_millis(5));
        report.finish(watch);
        assert!(
            report.duration_ms >= 1,
            "duration was {} ms",
            report.duration_ms
        );
    }

    #[test]
    fn json_shape_is_stable() {
        let json = sample().to_json();
        assert_eq!(json["files_removed"], 2);
        assert_eq!(json["reclaimed_bytes"], 157);
        assert_eq!(json["skipped"], 1);
        assert_eq!(json["errors"], 0);
        assert_eq!(json["aborted"], false);
        assert!(json["entries"].is_array());
        assert!(json["failures"].is_array());
    }

    /// A summary report is what keeps `preview --all` from shipping one JSON
    /// object per matched file: the counters must stay exact, the per-option
    /// totals must be exact, and only `entries` may be trimmed.
    #[test]
    fn summary_json_keeps_totals_and_caps_entries() {
        let report = sample();
        let json = report.to_json_summary(1);

        // Counters are untouched by the sample.
        assert_eq!(json["files_removed"], 2);
        assert_eq!(json["reclaimed_bytes"], 157);
        assert_eq!(json["items_count"], 4);
        assert_eq!(json["entries"].as_array().unwrap().len(), 1);
        assert_eq!(json["entries_truncated"], true);

        // firefox.cache = Delete (100 B) + Shred (50 B) + Skip (0 B) → 2 files,
        // 150 B; firefox.history = one Vacuum, which is not a removed file.
        let by = json["by_option"].as_array().unwrap();
        assert_eq!(by.len(), 2);
        assert_eq!(by[0]["cleaner"], "firefox");
        assert_eq!(by[0]["option"], "cache");
        assert_eq!(by[0]["files"], 2);
        assert_eq!(by[0]["bytes"], 150);
        assert_eq!(by[1]["option"], "history");
        assert_eq!(by[1]["files"], 0);
        assert_eq!(by[1]["bytes"], 7);

        // A cap wider than the report keeps every entry and says so.
        let full = sample().to_json_summary(10);
        assert_eq!(full["entries"].as_array().unwrap().len(), 4);
        assert_eq!(full["entries_truncated"], false);
    }

    #[test]
    fn entry_keeps_optional_path() {
        let entry = Entry::new(EntryKind::Command, "a", "b", "label", None, 0);
        assert_eq!(entry.path, None);
        assert_eq!(
            Entry::new(EntryKind::Command, "a", "b", "l", Some(Path::new("/x")), 0).path,
            Some(PathBuf::from("/x"))
        );
    }
}
