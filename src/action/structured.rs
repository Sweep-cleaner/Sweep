//! In-place editing of structured files: `ini` and `json`.
//!
//! These never delete a whole file — they remove a section, a parameter or a
//! JSON key, preserving everything else (and, for INI, the file's comments and
//! ordering). That matches BleachBit's behaviour and is what makes actions like
//! "reset the Firefox or Chrome configuration" safe.

use crate::action::context::RunContext;
use crate::core::error::{Error, Result};
use crate::core::report::{Entry, EntryKind, Report};
use crate::definition::model::{ActionDef, SearchKind, VarSet};
use crate::fsutil::walk::{glob_paths, scan_paths, ScanOptions};

/// Run an `ini` or `json` action. `def.section`/`def.parameter` (INI) and
/// `def.address` (JSON) select what to remove.
pub fn run(
    def: &ActionDef,
    vars: &VarSet,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
) -> Report {
    let mut report = Report::new();
    let command = def.command.as_str();

    for root in crate::definition::model::expand_action_paths(def, vars) {
        let paths = match def.search {
            SearchKind::File => {
                if root.exists() {
                    vec![root.to_path_buf()]
                } else {
                    Vec::new()
                }
            }
            SearchKind::Glob => glob_paths(&root.to_string_lossy()).unwrap_or_default(),
            _ => scan_paths(&root, &ScanOptions::files().with_same_filesystem(true)),
        };

        for path in paths {
            if ctx.cancelled() {
                report.aborted = true;
                return report;
            }
            // Keep list, protected roots and links all refuse in-place edits.
            if ctx.guard.check(&path, false).is_err() {
                report.push(Entry::skipped(cleaner, option, &path));
                continue;
            }

            let res = if command == "json" {
                edit_json(&path, def.address.as_deref(), ctx.dry_run)
            } else {
                edit_ini(
                    &path,
                    def.section.as_deref(),
                    def.parameter.as_deref(),
                    ctx.dry_run,
                )
            };

            match res {
                Ok(changed) if changed => report.push(Entry::new(
                    EntryKind::CleanFile,
                    cleaner,
                    option,
                    format!("edit {}", path.display()),
                    Some(path.as_path()),
                    0,
                )),
                Ok(_) => {} // nothing removed, not worth a line
                Err(err) => report.fail(cleaner, option, err.to_string()),
            }
        }
    }

    report
}

/// Remove a section or parameter from an INI file, preserving order and comments.
fn edit_ini(
    path: &std::path::Path,
    section: Option<&str>,
    parameter: Option<&str>,
    dry_run: bool,
) -> Result<bool> {
    let section = section.ok_or_else(|| {
        Error::msg(format!(
            "ini action on {} is missing a 'section'",
            path.display()
        ))
    })?;

    let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;

    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut changed = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let name = &trimmed[1..trimmed.len() - 1];
            in_section = name.eq_ignore_ascii_case(section);
            out.push(line.to_string());
            continue;
        }

        if in_section {
            if let Some(param) = parameter {
                if let Some((key, _)) = line.split_once('=') {
                    if key.trim().eq_ignore_ascii_case(param) {
                        changed = true; // drop this line
                        continue;
                    }
                }
                out.push(line.to_string());
            } else {
                // Whole-section removal.
                changed = true;
                continue;
            }
        } else {
            out.push(line.to_string());
        }
    }

    if !changed {
        return Ok(false);
    }

    if !dry_run {
        let mut new_text = out.join("\n");
        if text.ends_with('\n') {
            new_text.push('\n');
        }
        write_atomic(path, new_text)?;
    }
    Ok(true)
}

/// Remove a key (slash-separated `address`) from a JSON document.
fn edit_json(path: &std::path::Path, address: Option<&str>, dry_run: bool) -> Result<bool> {
    let address = address.ok_or_else(|| {
        Error::msg(format!(
            "json action on {} is missing an 'address'",
            path.display()
        ))
    })?;

    let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
    let mut value: serde_json::Value =
        serde_json::from_str(&text).map_err(|source| Error::Structured {
            path: path.to_path_buf(),
            reason: source.to_string(),
        })?;

    let mut keys: Vec<&str> = address.split('/').filter(|k| !k.is_empty()).collect();
    if keys.is_empty() {
        return Err(Error::msg(format!(
            "json address '{address}' does not point at a key"
        )));
    }
    let last = keys.pop().unwrap();

    let mut current = &mut value;
    for key in &keys {
        match current.get_mut(*key) {
            Some(next) => current = next,
            None => return Ok(false), // intermediate missing: nothing to do
        }
    }

    let changed = match current {
        serde_json::Value::Object(map) => map.remove(last).is_some(),
        _ => false,
    };

    if !changed {
        return Ok(false);
    }

    if !dry_run {
        let new_text =
            serde_json::to_string_pretty(&value).map_err(|source| Error::Structured {
                path: path.to_path_buf(),
                reason: source.to_string(),
            })?;
        write_atomic(path, new_text)?;
    }
    Ok(true)
}

/// Write `text` to `path`, replacing the original only after the temp write succeeds.
fn write_atomic(path: &std::path::Path, text: String) -> Result<()> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    // pid + counter: parallel options may edit different files in the same
    // directory, and two actions may target the same file.
    let mut temp = dir.to_path_buf();
    temp.push(format!(
        ".sweep_{}_{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&temp, text).map_err(|source| Error::io(&temp, source))?;
    std::fs::rename(&temp, path).map_err(|source| Error::io(path, source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ini_removes_section() {
        let text = "[Foo]\nkey = 1\n\n[Bar]\nkeep = 2\n";
        // round-trip through a temp file to exercise the real path
        let dir = std::env::temp_dir();
        let path = dir.join(format!("sweep_ini_test_{}.ini", std::process::id()));
        std::fs::write(&path, text).unwrap();
        assert!(edit_ini(&path, Some("Foo"), None, false).unwrap());
        let out = std::fs::read_to_string(&path).unwrap();
        assert!(out.contains("[Bar]"));
        assert!(!out.contains("key = 1"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn json_removes_key() {
        let text = r#"{"a": {"b": 1, "c": 2}}"#;
        let dir = std::env::temp_dir();
        let path = dir.join(format!("sweep_json_test_{}.json", std::process::id()));
        std::fs::write(&path, text).unwrap();
        assert!(edit_json(&path, Some("a/b"), false).unwrap());
        let out: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(out["a"].get("b").is_none());
        assert_eq!(out["a"]["c"], 2);
        let _ = std::fs::remove_file(&path);
    }
}
