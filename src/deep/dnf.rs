//! DNF/YUM önbelleği (Fedora/RHEL): `/var/cache/dnf`, `/var/cache/yum`.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{have, PreviewItem};

pub fn cache_dirs() -> Vec<PathBuf> {
    ["/var/cache/dnf", "/var/cache/yum"]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

pub fn preview_items(lang: &crate::i18n::Lang) -> Vec<PreviewItem> {
    let mut out = Vec::new();
    for dir in cache_dirs() {
        let options = crate::fsutil::walk::ScanOptions::files().with_max_depth(8);
        for path in crate::fsutil::walk::scan_paths(&dir, &options) {
            out.push(PreviewItem::new(
                crate::i18n::et(lang, "DNF/YUM cache {}", &[&path.display().to_string()]),
                Some(path.clone()),
                crate::fsutil::size::size_of_or_zero(&path),
            ));
        }
    }
    out
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    let tool = if have("dnf") {
        "dnf"
    } else if have("yum") {
        "yum"
    } else {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["dnf/yum (Fedora/RHEL?)"]),
        );
        return;
    };
    let items = preview_items(&ctx.lang);
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
                    &format!("{tool} clean all"),
                    &crate::deep::safety::human(total),
                ],
            ),
            None,
            0,
        ));
        return;
    }
    match crate::fsutil::run_command(tool, &["clean", "all"], true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "finished: {} (~{} cleaned)",
                &[
                    &format!("{tool} clean all"),
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
                &[&format!("{tool} clean all"), &code.to_string(), &stderr],
            ),
        ),
        Err(err) => report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
        ),
    }
}
