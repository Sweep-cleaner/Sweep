//! Action dispatch.
//!
//! A cleaner definition names an action command (`delete`, `sqlite.vacuum`,
//! `system.trash`, ...). This module maps that name onto the concrete
//! implementation in one of the sub-modules and runs it under a [`RunContext`].

pub mod context;
pub mod file;
pub mod filter;
pub mod provider;
pub mod sqlite;
pub mod structured;
pub mod system;

pub use context::RunContext;

use crate::core::report::Report;
use crate::definition::model::{ActionDef, VarSet};

/// Run a single action, returning every entry it produced (and any non-fatal
/// failures, which are recorded inside the [`Report`]).
pub fn run(
    def: &ActionDef,
    vars: &VarSet,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
) -> Report {
    // `search="deep"` is handled by the deep-scan engine, not here.
    if def.search == crate::definition::model::SearchKind::Deep {
        return Report::new();
    }

    match def.command.as_str() {
        "delete" | "shred" | "truncate" => file::run(def, vars, ctx, cleaner, option),
        "ini" | "json" => structured::run(def, vars, ctx, cleaner, option),
        cmd if cmd.starts_with("sqlite.") => sqlite::run(def, vars, ctx, cleaner, option),
        // Browser profile databases (Chrome/Chromium/Edge/Brave/Vivaldi and
        // Firefox/Mozilla) are SQLite files. These commands are implemented in
        // the sqlite module via `delete_statements`, so route them there even
        // though they are not `sqlite.`-prefixed.
        cmd if cmd.starts_with("chrome.") || cmd.starts_with("mozilla.") || cmd == "cookies" => {
            sqlite::run(def, vars, ctx, cleaner, option)
        }
        cmd if cmd.starts_with("system.") => system::run(def, vars, ctx, cleaner, option),
        "process" | "winreg" => system::run(def, vars, ctx, cleaner, option),
        // Package-manager commands are implemented in the system module.
        "apt.clean" | "apt.autoclean" | "apt.autoremove" | "dnf.clean" | "yum.clean"
        | "pacman.clean" | "zypper.clean" | "brew.cleanup" => {
            system::run(def, vars, ctx, cleaner, option)
        }
        other => {
            // Unknown command: report it, but do not crash the whole cleaner.
            let mut report = Report::new();
            report.fail(cleaner, option, format!("unknown action command '{other}'"));
            report
        }
    }
}
