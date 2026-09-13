//! Snap kalıntıları: devre dışı bırakılmış revizyonlar + indirme önbelleği.
//!
//! `snap list --all` çıktısında `disabled` satırlar hedeflenir; güncel
//! revizyonlara dokunulmaz. Önbellek: `/var/lib/snapd/cache`.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// (uygulama, revizyon): devre dışı snap'ler.
pub fn disabled_revisions() -> Vec<(String, String)> {
    let Some(out) = command_output("snap", &["list", "--all"]) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for line in out.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() >= 6 && cols[5] == "disabled" {
            found.push((cols[0].to_string(), cols[2].to_string()));
        }
    }
    found
}

pub fn cache_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from("/var/lib/snapd/cache")];
    if let Some(home) = crate::platform::home_dir() {
        let p = home.join("snap");
        if p.is_dir() {
            // ~/snap altında yalnızca önbellek benzeri alt dizinler.
            let d = p.join("common/.cache");
            if d.is_dir() {
                dirs.push(d);
            }
        }
    }
    dirs.into_iter().filter(|p| p.is_dir()).collect()
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !have("snap") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["snap"]),
        );
        return;
    }
    let disabled = disabled_revisions();
    let caches = cache_dirs();

    if ctx.dry_run {
        for (app, rev) in &disabled {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "disabled snap to remove: {} revision {}",
                    &[&app, &rev],
                ),
                None,
                0,
            ));
        }
        for dir in &caches {
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "snap cache {}", &[&dir.display().to_string()]),
                Some(dir),
                crate::fsutil::size::dir_size(dir),
            ));
        }
        if disabled.is_empty() && caches.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no snap leftovers"),
                None,
                0,
            ));
        }
        return;
    }
    if !crate::deep::safety::is_root() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "requires root"));
        return;
    }
    for (app, rev) in &disabled {
        match crate::fsutil::run_command("snap", &["remove", app, "--revision", rev], true) {
            Ok((0, _, _)) => report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "removed: {} rev {}", &[&app, &rev]),
                None,
                0,
            )),
            Ok((code, _, stderr)) => report.fail(
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "{} exited {}: {}",
                    &[&format!("snap remove {app}"), &code.to_string(), &stderr],
                ),
            ),
            Err(err) => report.fail(
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
            ),
        }
    }
    for dir in &caches {
        crate::deep::clean_dir_contents(dir, ctx, cleaner, option, report, 8);
    }
}
