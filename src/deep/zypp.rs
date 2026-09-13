//! Zypper önbelleği (openSUSE): `/var/cache/zypp`.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{have, PreviewItem};

pub fn cache_dirs() -> Vec<PathBuf> {
    ["/var/cache/zypp"]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !have("zypper") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["zypper (openSUSE?)"]),
        );
        return;
    }
    let mut items = Vec::new();
    for dir in cache_dirs() {
        let options = crate::fsutil::walk::ScanOptions::files().with_max_depth(8);
        for path in crate::fsutil::walk::scan_paths(&dir, &options) {
            items.push(PreviewItem::new(
                crate::i18n::et(&ctx.lang, "zypper cache {}", &[&path.display().to_string()]),
                Some(path.clone()),
                crate::fsutil::size::size_of_or_zero(&path),
            ));
        }
    }
    let total: u64 = items.iter().map(|i| i.bytes).sum();
    if ctx.dry_run {
        crate::deep::push_preview(report, cleaner, option, items);
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "will run: {} (~{} reclaimable)",
                &[
                    &"zypper clean --all".to_string(),
                    &crate::deep::safety::human(total),
                ],
            ),
            None,
            0,
        ));
        return;
    }
    match crate::fsutil::run_command("zypper", &["clean", "--all"], true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "finished: {} (~{} cleaned)",
                &[
                    &"zypper clean --all".to_string(),
                    &crate::deep::safety::human(total),
                ],
            ),
            None,
            total,
        )),
        Ok((code, _, stderr)) => report.fail(
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "{} exited {}: {}",
                &[&"zypper".to_string(), &code.to_string(), &stderr],
            ),
        ),
        Err(err) => report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
        ),
    }
}
