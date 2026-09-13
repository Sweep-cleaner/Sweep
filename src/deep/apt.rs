//! APT önbelleği (Debian/Ubuntu): `/var/cache/apt/archives` + `apt-get`.
//!
//! Derin tarama iki katmanlıdır: önbellek dizinindeki `.deb` dosyaları boyut
//! bazında önizlenir; gerçek temizlikte `apt-get clean/autoclean` çalışır.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have, PreviewItem};

/// APT önbellek dizinleri.
pub fn cache_dirs() -> Vec<PathBuf> {
    ["/var/cache/apt/archives", "/var/cache/apt/archives/partial"]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect()
}

/// Önizleme: her `.deb` dosyası + toplam.
pub fn preview_items(lang: &crate::i18n::Lang) -> Vec<PreviewItem> {
    let mut out = Vec::new();
    for dir in cache_dirs() {
        let options = crate::fsutil::walk::ScanOptions::files();
        for path in crate::fsutil::walk::scan_paths(&dir, &options) {
            if path.extension().map(|e| e == "deb").unwrap_or(false) {
                out.push(PreviewItem::new(
                    crate::i18n::et(lang, "APT package {}", &[&path.display().to_string()]),
                    Some(path.clone()),
                    crate::fsutil::size::size_of_or_zero(&path),
                ));
            }
        }
    }
    out
}

/// `apt-get clean` / `autoclean` çalıştır (dry-run'da önizleme kaydı).
pub fn execute(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    autoclean: bool,
) {
    if !have("apt-get") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "tool not found: {}",
                &["apt-get (Debian/Ubuntu?)"],
            ),
        );
        return;
    }
    let items = preview_items(&ctx.lang);
    let total: u64 = items.iter().map(|i| i.bytes).sum();
    if ctx.dry_run {
        crate::deep::push_preview(report, cleaner, option, items);
        let cmd = if autoclean {
            "apt-get autoclean"
        } else {
            "apt-get clean"
        };
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "will run: {} (~{} reclaimable)",
                &[cmd.to_string(), crate::deep::safety::human(total)],
            ),
            None,
            total,
        ));
        return;
    }
    let cmd = if autoclean { "autoclean" } else { "clean" };
    match crate::fsutil::run_command("apt-get", &[cmd, "-y"], true) {
        Ok((0, _, _)) => {
            for item in &items {
                if let Some(p) = &item.path {
                    crate::deep::safety::log_operation(ctx, cleaner, option, p, item.bytes, false);
                }
            }
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "finished: apt-get {} (~{} cleaned)",
                    &[&cmd.to_string(), &crate::deep::safety::human(total)],
                ),
                None,
                total,
            ));
        }
        Ok((code, _, stderr)) => report.fail(
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "{} exited {}: {}",
                &[&format!("apt-get {cmd}"), &code.to_string(), &stderr],
            ),
        ),
        Err(err) => report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
        ),
    }
}

/// Paket listeleri (`/var/lib/apt/lists`): `apt-get update` ile yeniden iner.
pub fn execute_lists(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    use crate::core::report::{Entry, EntryKind};
    use std::path::PathBuf;

    let root = PathBuf::from("/var/lib/apt/lists");
    if !root.is_dir() || !have("apt-get") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "tool not found: {}",
                &["APT lists (Debian/Ubuntu?)"],
            ),
        );
        return;
    }
    let options = crate::fsutil::walk::ScanOptions::files().with_max_depth(2);
    let files = crate::fsutil::walk::scan_paths(&root, &options);
    let total: u64 = files
        .iter()
        .map(|p| crate::fsutil::size::size_of_or_zero(p))
        .sum();
    if ctx.dry_run {
        for path in &files {
            // Kilit ve kısmi dosyalar korunur.
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name == "lock" || name == "partial" {
                continue;
            }
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "APT list {}", &[&path.display().to_string()]),
                Some(path),
                crate::fsutil::size::size_of_or_zero(path),
            ));
        }
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "~{} package lists (regenerated by apt-get update)",
                &[&crate::deep::safety::human(total)],
            ),
            None,
            0,
        ));
        return;
    }
    if !crate::deep::safety::is_root() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "requires root"));
        return;
    }
    for path in &files {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name == "lock" || name == "partial" {
            continue;
        }
        crate::deep::guarded_delete(path, ctx, cleaner, option, report);
    }
}

/// Eski çekirdek listesi (APT tabanlı) — kernel.rs ile paylaşılır.
pub fn installed_kernels() -> Vec<String> {
    let Some(out) = command_output("dpkg", &["--list", "linux-image-*"]) else {
        return Vec::new();
    };
    out.lines()
        .filter(|l| l.starts_with("ii"))
        .filter_map(|l| l.split_whitespace().nth(1))
        .map(|s| s.to_string())
        .collect()
}
