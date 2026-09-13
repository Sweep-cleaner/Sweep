//! Güvenli silme komutu: dosyaların üzerine yazıp siler (`sweep shred`).
//!
//! `rm`'den farkı: içerik geri getirilemesin diye silmeden önce
//! `ShredSpec` ile üzerine yazılır. Dizinler kabul edilmez (önce içini
//! `clean` ile boşaltın); kritik dizin/guard kontrolleri aynen geçer.

use std::path::PathBuf;

use crate::action::context::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "shred";

/// Tara ve raporla; `ctx.dry_run == false` ise üzerine yazıp sil.
pub fn scan(paths: &[PathBuf], ctx: &RunContext) -> Report {
    let mut report = Report::new();
    if !ctx.dry_run && !paths.is_empty() {
        crate::deep::safety::log_run_start(ctx, "shred");
    }
    if paths.is_empty() {
        report.fail(
            CLEANER_ID,
            "args",
            crate::i18n::t(&ctx.lang, "no files given"),
        );
        return report;
    }
    for path in paths {
        if path.is_dir() {
            report.fail(
                CLEANER_ID,
                "args",
                crate::i18n::et(
                    &ctx.lang,
                    "cannot shred dir: {} (clean it first)",
                    &[&path.display().to_string()],
                ),
            );
            continue;
        }
        if let Err(reason) = crate::deep::safety::check_path(path, ctx) {
            report.push(Entry::skipped(CLEANER_ID, "guard", path));
            report.fail(
                CLEANER_ID,
                "guard",
                crate::i18n::et(&ctx.lang, "{}: {}", &[&path.display().to_string(), &reason]),
            );
            continue;
        }
        let bytes = crate::fsutil::size::size_of_or_zero(path);
        if ctx.dry_run {
            report.push(Entry::new(
                EntryKind::Shred,
                CLEANER_ID,
                "shred",
                crate::i18n::et(&ctx.lang, "to shred {}", &[&path.display().to_string()]),
                Some(path),
                bytes,
            ));
            continue;
        }
        crate::deep::safety::maybe_backup(ctx, path);
        match crate::shred::shred_file(path, ctx.shred_spec) {
            Ok(()) => {
                report.push(Entry::new(
                    EntryKind::Shred,
                    CLEANER_ID,
                    "shred",
                    crate::i18n::et(&ctx.lang, "shredded {}", &[&path.display().to_string()]),
                    Some(path),
                    bytes,
                ));
                crate::deep::safety::log_operation(ctx, CLEANER_ID, "shred", path, bytes, false);
            }
            Err(err) if err.is_benign() => {}
            Err(err) => report.fail(
                CLEANER_ID,
                "shred",
                crate::i18n::et(
                    &ctx.lang,
                    "{}: {}",
                    &[&path.display().to_string(), &err.to_string()],
                ),
            ),
        }
    }
    report
}
