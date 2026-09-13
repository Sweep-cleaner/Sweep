//! Pacman önbelleği (Arch): `/var/cache/pacman/pkg` + `paccache`.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{have, PreviewItem};

pub fn cache_dirs() -> Vec<PathBuf> {
    ["/var/cache/pacman/pkg"]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

pub fn preview_items(lang: &crate::i18n::Lang) -> Vec<PreviewItem> {
    let mut out = Vec::new();
    for dir in cache_dirs() {
        let options = crate::fsutil::walk::ScanOptions::files();
        for path in crate::fsutil::walk::scan_paths(&dir, &options) {
            out.push(PreviewItem::new(
                crate::i18n::et(lang, "pacman package {}", &[&path.display().to_string()]),
                Some(path.clone()),
                crate::fsutil::size::size_of_or_zero(&path),
            ));
        }
    }
    out
}

/// `keep` son sürümü tutar (varsayılan 2); `paccache -r` varsa onu kullanır.
pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report, keep: usize) {
    if !have("pacman") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["pacman (Arch?)"]),
        );
        return;
    }
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
                    &format!("paccache -r -k {keep}"),
                    &crate::deep::safety::human(total),
                ],
            ),
            None,
            0,
        ));
        return;
    }
    if have("paccache") {
        let k = keep.to_string();
        match crate::fsutil::run_command("paccache", &["-r", "-k", &k], true) {
            Ok((0, _, _)) => {
                report.push(Entry::new(
                    EntryKind::Command,
                    cleaner,
                    option,
                    crate::i18n::et(
                        &ctx.lang,
                        "finished: {}",
                        &[&format!("paccache -r -k {keep}")],
                    ),
                    None,
                    total,
                ));
                return;
            }
            Ok((code, _, stderr)) => {
                report.fail(
                    cleaner,
                    option,
                    crate::i18n::et(
                        &ctx.lang,
                        "{} exited {}: {}",
                        &[&"paccache".to_string(), &code.to_string(), &stderr],
                    ),
                );
                return;
            }
            Err(err) => {
                report.fail(
                    cleaner,
                    option,
                    crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
                );
                return;
            }
        }
    }
    // paccache yoksa: pacman -Scc (tüm önbellek; uyarı cleaner tanımında).
    match crate::fsutil::run_command("pacman", &["-Scc", "--noconfirm"], true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "finished: {} (~{} cleaned)",
                &[
                    &"pacman -Scc".to_string(),
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
                &[&"pacman -Scc".to_string(), &code.to_string(), &stderr],
            ),
        ),
        Err(err) => report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
        ),
    }
}
