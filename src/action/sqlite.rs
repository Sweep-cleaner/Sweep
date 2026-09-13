//! SQLite-backed actions for browsers and other embedded databases.
//!
//! Ported from BleachBit's `Special.py` (`delete_chrome_history`,
//! `delete_mozilla_url_history`, `delete_mozilla_cookies`, ...) with one safety
//! upgrade: a locked or read-only database is copied to a temporary file,
//! operated on there, and only then swapped back over the original — so a
//! running browser no longer turns a cleaning run into a wall of errors.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::action::context::RunContext;
use crate::core::error::{Error, Result};
use crate::core::report::{Entry, EntryKind, Report};
use crate::definition::model::{ActionDef, VarSet};
use crate::fsutil::size::size_of;
use crate::fsutil::walk::{scan_paths, ScanOptions};

/// DELETE statements per command, run in order inside a transaction.
///
/// Statements that fail (unknown table) are ignored so an action is forward
/// compatible across Chrome/Firefox versions.
fn delete_statements(command: &str) -> &'static [&'static str] {
    match command {
        "chrome.history" => &[
            "DELETE FROM urls",
            "DELETE FROM visits",
            "DELETE FROM visit_source",
            "DELETE FROM keyword_search_terms",
            "DELETE FROM segments",
            "DELETE FROM segment_usage",
            "DELETE FROM downloads",
            "DELETE FROM downloads_url_chains",
            "DELETE FROM downloads_slices",
        ],
        "chrome.keywords" => &["DELETE FROM keyword_search_terms"],
        // Otomatik doldurma profilleri (kredi kartları DAHİL DEĞİL — korunur).
        "chrome.autofill" => &[
            "DELETE FROM autofill_profiles",
            "DELETE FROM autofill_profile_emails",
            "DELETE FROM autofill_profile_phones",
        ],
        "chrome.favicons" => &[
            "DELETE FROM icon_mapping",
            "DELETE FROM favicon_bitmaps",
            "DELETE FROM favicons",
        ],
        "chrome.databases_db" => &["DELETE FROM databases", "DELETE FROM meta"],
        "chrome.cookies" | "cookies" => &["DELETE FROM cookies"],
        "mozilla.history" | "mozilla.url_history" => &[
            "DELETE FROM moz_historyvisits",
            "DELETE FROM moz_inputhistory",
            "DELETE FROM moz_places WHERE id NOT IN (SELECT fk FROM moz_bookmarks WHERE fk IS NOT NULL)",
        ],
        "mozilla.databases" => &[
            "DELETE FROM moz_places WHERE last_visit_date IS NULL",
            "DELETE FROM moz_historyvisits WHERE visit_date IS NULL",
        ],
        _ => &[],
    }
}

/// Run a SQLite action. `def.path` is one database, or (for `mozilla.vacuum`)
/// a profile directory whose `*.sqlite` files are all vacuumed.
pub fn run(
    def: &ActionDef,
    vars: &VarSet,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
) -> Report {
    let mut report = Report::new();

    for root in crate::definition::model::expand_action_paths(def, vars) {
        if def.command == "mozilla.vacuum"
            || def.search != crate::definition::model::SearchKind::File
        {
            vacuum_directory(&root, ctx, cleaner, option, &mut report);
            continue;
        }

        if ctx.guard.check(&root, false).is_err() {
            report.push(Entry::skipped(cleaner, option, &root));
            continue;
        }
        let before = size_of(&root).unwrap_or(0);
        let reclaimed = match operate(&root, def.command.as_str(), ctx.dry_run) {
            Ok(freed) => freed,
            Err(err) => {
                report.fail(cleaner, option, err.to_string());
                continue;
            }
        };
        let after = size_of(&root).unwrap_or(before);
        let reclaimed = reclaimed + (before.saturating_sub(after));
        report.push(Entry::new(
            EntryKind::Vacuum,
            cleaner,
            option,
            format!("clean {}", root.display()),
            Some(root.as_path()),
            reclaimed,
        ));
    }

    report
}

/// VACUUM every `*.sqlite` database under `dir`.
fn vacuum_directory(
    dir: &Path,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    if !dir.is_dir() {
        return;
    }
    let options = ScanOptions::files().with_max_depth(4);
    for db in scan_paths(dir, &options) {
        if db
            .extension()
            .map(|e| e.eq_ignore_ascii_case("sqlite"))
            .unwrap_or(false)
        {
            if ctx.cancelled() {
                report.aborted = true;
                return;
            }
            if ctx.guard.check(&db, false).is_err() {
                report.push(Entry::skipped(cleaner, option, &db));
                continue;
            }
            let before = size_of(&db).unwrap_or(0);
            let reclaimed = operate(&db, "sqlite.vacuum", ctx.dry_run).unwrap_or(0)
                + before.saturating_sub(size_of(&db).unwrap_or(before));
            report.push(Entry::new(
                EntryKind::Vacuum,
                cleaner,
                option,
                format!("vacuum {}", db.display()),
                Some(db.as_path()),
                reclaimed,
            ));
        }
    }
}

/// Open a database read-write, falling back to a temp copy when it is locked.
fn open_writable(path: &Path) -> Result<(Connection, Option<(PathBuf, PathBuf)>)> {
    match Connection::open(path) {
        Ok(conn) => Ok((conn, None)),
        Err(_) => {
            let mut temp = std::env::temp_dir();
            temp.push(format!(
                "sweep_{}_{}.sqlite",
                path.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default(),
                std::process::id()
            ));
            match std::fs::copy(path, &temp) {
                Ok(_) => match Connection::open(&temp) {
                    Ok(conn) => Ok((conn, Some((temp, path.to_path_buf())))),
                    Err(source) => Err(Error::Database {
                        path: path.to_path_buf(),
                        source,
                    }),
                },
                Err(source) => Err(Error::io(path, source)),
            }
        }
    }
}

/// One database operation. Returns bytes reclaimed (best effort).
///
/// A preview never touches the database: `reclaimed` is derived by the caller
/// from the before/after sizes (which are equal), so it reports zero.
fn operate(path: &Path, command: &str, dry_run: bool) -> Result<u64> {
    if dry_run {
        return Ok(0);
    }
    let (conn, swap) = open_writable(path)?;

    if command == "sqlite.vacuum" {
        conn.execute_batch("VACUUM;")
            .map_err(|source| Error::Database {
                path: path.to_path_buf(),
                source,
            })?;
        drop(conn);
        return Ok(0);
    }

    let statements = delete_statements(command);
    if !statements.is_empty() {
        for stmt in statements {
            // Best-effort: a table may not exist in this version of the DB.
            let _ = conn.execute_batch(stmt);
        }
        // VACUUM actually releases the space left behind by the deletes.
        let _ = conn.execute_batch("VACUUM;");
    }

    drop(conn);

    if let Some((temp, original)) = swap {
        // The temp copy may live on another filesystem (`/tmp`), where a
        // rename cannot work — fall back to copy-and-remove.
        if std::fs::rename(&temp, &original).is_err() {
            std::fs::copy(&temp, &original).map_err(|source| Error::io(&original, source))?;
            let _ = std::fs::remove_file(&temp);
        }
    }

    Ok(0)
}
