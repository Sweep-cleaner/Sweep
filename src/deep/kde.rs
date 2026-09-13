//! KDE/Plasma önbellekleri: güvenli plazma yaprakları, Baloo dizini, Akonadi.
//!
//! Dokunulmayanlar: KWallet, yapılandırma (`~/.config`), `drkonqi` (çalışan
//! süreç inode tuttuğu için `builtin_keep_list` korur), son belgeler.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// Temizlik kapsamı (TOML'daki her seçenek bir kapsama bağlanır).
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    /// Plazma kabuğu, tema, SVG, simge, KRunner, KIO, sycoca.
    Plasma,
    /// Akonadi dosya önbelleği (veritabanına dokunulmaz).
    Pim,
}

/// `~/.cache` altındaki güvenli plazma yaprakları.
const PLASMA_LEAVES: &[&str] = &[
    "plasmashell",
    "plasma-svgelements",
    "plasma_theme_default_v2",
    "plasmoids",
    "kwin",
    "krunner",
    "kioexec",
    "ksvg",
    "ksycoca5",
    "ksycoca6",
    "icon-cache.kcache",
    "kio_http_cache",
    "kio_http_cache_cleaner",
    "attica-kde",
    "discover",
    "gwenview",
    "dolphin",
    "dolphinpg",
    "konsole",
    "kate",
    "okular",
    "spectacle",
    "kglobalaccel",
    "knotifications",
    "plasma_engine_dict",
    "plasma_applet_dict",
];

/// `~/.local/share` altındaki güvenli plazma yaprakları.
const PLASMA_LOCAL_LEAVES: &[&str] = &["sycoca", "kracked", "RecentDocuments"];

/// Akonadi dosya önbelleği (DB dizinleri hariç).
const PIM_LEAVES: &[&str] = &["akonadi/file_db_data"];

fn home() -> Option<PathBuf> {
    crate::platform::home_dir()
}

fn existing(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter(|p| p.is_dir() || p.is_file())
        .collect()
}

/// Kapsama giren hedef yollar.
pub fn targets(scope: Scope) -> Vec<PathBuf> {
    let Some(h) = home() else {
        return Vec::new();
    };
    let cache = crate::platform::cache_dir().unwrap_or_else(|| h.join(".cache"));
    let local = h.join(".local").join("share");
    match scope {
        Scope::Plasma => {
            let mut v: Vec<PathBuf> = PLASMA_LEAVES.iter().map(|l| cache.join(l)).collect();
            v.extend(PLASMA_LOCAL_LEAVES.iter().map(|l| local.join(l)));
            // ksycoca sürüm sonekli dosyalar (ksycoca5_xxx gibi).
            if cache.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&cache) {
                    for e in entries.flatten() {
                        let n = e.file_name().to_string_lossy().into_owned();
                        if n.starts_with("ksycoca") || n.starts_with("kcache") {
                            v.push(e.path());
                        }
                    }
                }
            }
            existing(v)
        }
        Scope::Pim => existing(PIM_LEAVES.iter().map(|l| local.join(l)).collect()),
    }
}

/// Baloo dizin boyutu (önizleme için).
pub fn baloo_size() -> u64 {
    let Some(h) = home() else {
        return 0;
    };
    let idx = h.join(".local").join("share").join("baloo");
    if idx.is_dir() {
        crate::fsutil::size::dir_size(&idx)
    } else {
        0
    }
}

/// Baloo etkin mi?
pub fn baloo_available() -> bool {
    have("balooctl")
        || home()
            .map(|h| h.join(".local/share/baloo").is_dir())
            .unwrap_or(false)
}

pub fn execute_cache(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    scope: Scope,
) {
    let items: Vec<crate::deep::PreviewItem> = targets(scope)
        .into_iter()
        .map(|p| {
            let bytes = if p.is_dir() {
                crate::fsutil::size::dir_size(&p)
            } else {
                crate::fsutil::size::size_of_or_zero(&p)
            };
            crate::deep::PreviewItem::new(
                crate::i18n::et(&ctx.lang, "KDE cache {}", &[&p.display().to_string()]),
                Some(p),
                bytes,
            )
        })
        .collect();
    if ctx.dry_run {
        crate::deep::push_preview(report, cleaner, option, items);
        return;
    }
    for item in &items {
        if let Some(path) = &item.path {
            if path.is_dir() {
                crate::deep::clean_dir_contents(path, ctx, cleaner, option, report, 16);
            } else {
                crate::deep::guarded_delete(path, ctx, cleaner, option, report);
            }
            crate::deep::safety::log_operation(ctx, cleaner, option, path, item.bytes, false);
        }
    }
}

/// Baloo: önce `balooctl purge` (daemon-güvenli), yoksa dizin süpürme.
pub fn execute_baloo(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !baloo_available() {
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::t(&ctx.lang, "no Baloo index found"),
            None,
            0,
        ));
        return;
    }
    let size = baloo_size();
    if ctx.dry_run {
        let method = if have("balooctl") {
            "balooctl purge"
        } else {
            "dir sweep"
        };
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "will run: {} (~{} reclaimable)",
                &[&method.to_string(), &crate::deep::safety::human(size)],
            ),
            None,
            size,
        ));
        if let Some(out) = command_output("balooctl", &["status"]) {
            for line in out.lines().take(6) {
                report.push(Entry::new(
                    EntryKind::Command,
                    cleaner,
                    option,
                    crate::i18n::et(&ctx.lang, "baloo: {}", &[&line.to_string()]),
                    None,
                    0,
                ));
            }
        }
        return;
    }
    if have("balooctl") {
        match crate::fsutil::run_command("balooctl", &["purge"], true) {
            Ok((0, _, _)) => {
                report.push(Entry::new(
                    EntryKind::Command,
                    cleaner,
                    option,
                    crate::i18n::et(
                        &ctx.lang,
                        "Baloo index purged (~{})",
                        &[&crate::deep::safety::human(size)],
                    ),
                    None,
                    size,
                ));
                crate::deep::safety::log_operation(
                    ctx,
                    cleaner,
                    option,
                    &home().unwrap_or_default().join(".local/share/baloo"),
                    size,
                    false,
                );
            }
            Ok((code, _, stderr)) => report.fail(
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "{} exited {}: {}",
                    &[&"balooctl purge".to_string(), &code.to_string(), &stderr],
                ),
            ),
            Err(err) => report.fail(
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
            ),
        }
        return;
    }
    if let Some(h) = home() {
        let dir = h.join(".local").join("share").join("baloo");
        crate::deep::clean_dir_contents(&dir, ctx, cleaner, option, report, 8);
    }
}
