//! Flatpak kalıntıları: kullanılmayan runtime'lar + önbellek dizinleri.
//!
//! `flatpak uninstall --unused` resmi yoldur; ek olarak `.removed` ve
//! appstream/snapcraft önbellekleri süpürülür.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

pub fn unused_list() -> Vec<String> {
    // Kuru çalıştırma çıktısını ayrıştır (destekleyen sürümlerde).
    if let Some(out) = command_output("flatpak", &["uninstall", "--unused", "--dry-run"]) {
        return out
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
    }
    Vec::new()
}

pub fn leftover_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "linux")]
    let mut dirs = crate::platform::linux::container_leftovers();
    #[cfg(not(target_os = "linux"))]
    let mut dirs: Vec<PathBuf> = Vec::new();
    for root in ["/var/lib/flatpak", "/var/tmp/flatpak-cache-*"] {
        for p in glob::paths(root) {
            if p.is_dir() {
                dirs.push(p);
            }
        }
    }
    if let Some(home) = crate::platform::home_dir() {
        for leaf in [
            ".var/app/.cache",
            ".local/share/flatpak/appstream/swcatalog",
        ] {
            let p = home.join(leaf);
            if p.is_dir() {
                dirs.push(p);
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

mod glob {
    use std::path::PathBuf;
    /// Basit glob: yalnızca sondaki `*` desteklenir.
    pub fn paths(pattern: &str) -> Vec<PathBuf> {
        if let Some(prefix) = pattern.strip_suffix('*') {
            let parent = std::path::Path::new(prefix)
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("/"));
            let stem = std::path::Path::new(prefix)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            std::fs::read_dir(parent)
                .map(|it| {
                    it.flatten()
                        .map(|e| e.path())
                        .filter(|p| {
                            p.file_name()
                                .map(|n| n.to_string_lossy().starts_with(stem.as_str()))
                                .unwrap_or(false)
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            let p = PathBuf::from(pattern);
            if p.exists() {
                vec![p]
            } else {
                Vec::new()
            }
        }
    }
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !have("flatpak") {
        // flatpak kurulu değilse bile kalıntı dizinler temizlenebilir.
        log::info!("no flatpak; scanning leftover dirs only");
    }
    let unused = unused_list();
    let leftovers = leftover_dirs();

    if ctx.dry_run {
        for u in &unused {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "unused runtime to remove: {}", &[&u]),
                None,
                0,
            ));
        }
        for dir in &leftovers {
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "flatpak leftover {}",
                    &[&dir.display().to_string()],
                ),
                Some(dir),
                crate::fsutil::size::dir_size(dir),
            ));
        }
        if unused.is_empty() && leftovers.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no flatpak leftovers"),
                None,
                0,
            ));
        }
        return;
    }
    if have("flatpak") {
        match crate::fsutil::run_command("flatpak", &["uninstall", "--unused", "-y"], true) {
            Ok((0, _, _)) => report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "flatpak unused removed"),
                None,
                0,
            )),
            Ok((code, _, stderr)) => report.fail(
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "{} exited {}: {}",
                    &[&"flatpak uninstall".to_string(), &code.to_string(), &stderr],
                ),
            ),
            Err(err) => report.fail(
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
            ),
        }
    }
    for dir in &leftovers {
        crate::deep::clean_dir_contents(dir, ctx, cleaner, option, report, 8);
    }
}
