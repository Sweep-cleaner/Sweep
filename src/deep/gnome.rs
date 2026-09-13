//! GNOME önbellekleri: güvenli uygulama yaprakları, Tracker dizini, Zeitgeist.
//!
//! Dokunulmayanlar: dconf (`~/.config/dconf`), GNOME Shell eklentileri
//! (`extensions/`), arka planlar ve anahtarlıklar. Zeitgeist etkinlik
//! günlüğüdür; ayrı, uyarılı bir seçenektir.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// Temizlik kapsamı (TOML'daki her seçenek bir kapsama bağlanır).
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    /// Yeniden üretilebilir GNOME uygulama önbellekleri.
    Apps,
    /// Zeitgeist etkinlik günlüğü (gizlilik).
    Zeitgeist,
}

/// `~/.cache` altındaki güvenli GNOME yaprakları.
const APP_LEAVES: &[&str] = &[
    "gnome-software",
    "gnome-control-center",
    "gnome-shell",
    "gnome-desktop",
    "gnome-initial-setup",
    "gdm",
    "gjs",
    "nautilus",
    "org.gnome.Nautilus",
    "file-roller",
    "org.gnome.TextEditor",
    "gnome-text-editor",
    "gnome-calculator",
    "gnome-calendar",
    "gnome-clocks",
    "gnome-maps",
    "gnome-weather",
    "gnome-music",
    "gnome-photos",
    "totem",
    "gnome-terminal",
    "gnome-console",
    "gnome-builder",
    "gnome-boxes",
    "gnome-connections",
    "gnome-logs",
    "gnome-font-viewer",
    "gnome-disk-utility",
    "deja-dup",
    "seahorse",
    "vinagre",
    "remmina",
    "evolution",
    "geary",
    "loupe",
    "snapshot",
    "showtime",
    "decibels",
    "papers",
];

/// `~/.local/share` altında önbellek sayılan yapraklar (eklentiler hariç).
const LOCAL_LEAVES: &[&str] = &[
    "gnome-shell/application_state",
    "gnome-settings-daemon",
    "nautilus/scripts",
];

fn home() -> Option<PathBuf> {
    crate::platform::home_dir()
}

/// Kapsama giren mevcut hedef yollar.
pub fn targets(scope: Scope) -> Vec<PathBuf> {
    let Some(h) = home() else {
        return Vec::new();
    };
    let cache = crate::platform::cache_dir().unwrap_or_else(|| h.join(".cache"));
    let local = h.join(".local").join("share");
    let leaves: &[&str] = match scope {
        Scope::Apps => APP_LEAVES,
        Scope::Zeitgeist => &["zeitgeist"],
    };
    let mut v: Vec<PathBuf> = leaves.iter().map(|l| cache.join(l)).collect();
    if matches!(scope, Scope::Apps) {
        v.extend(LOCAL_LEAVES.iter().map(|l| local.join(l)));
        // Sürüm sonekli GNOME önbellekleri (gnome-shell-42 gibi).
        if cache.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&cache) {
                for e in entries.flatten() {
                    let n = e.file_name().to_string_lossy().into_owned();
                    if n.starts_with("gnome-shell-") || n.starts_with("gdm-") {
                        v.push(e.path());
                    }
                }
            }
        }
    }
    v.into_iter()
        .filter(|p| p.is_dir() || p.is_file())
        .collect()
}

/// Tracker dizin boyutu (önizleme için).
pub fn tracker_size() -> u64 {
    let Some(h) = home() else {
        return 0;
    };
    let mut total = 0;
    for leaf in ["tracker", "tracker3"] {
        let d = h.join(".cache").join(leaf);
        if d.is_dir() {
            total += crate::fsutil::size::dir_size(&d);
        }
        let d = h.join(".local").join("share").join(leaf);
        if d.is_dir() {
            total += crate::fsutil::size::dir_size(&d);
        }
    }
    total
}

pub fn tracker_available() -> bool {
    have("tracker3") || have("tracker") || tracker_size() > 0
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
            let what = match scope {
                Scope::Apps => "GNOME cache",
                Scope::Zeitgeist => "Zeitgeist log",
            };
            crate::deep::PreviewItem::new(format!("{what} {}", p.display()), Some(p), bytes)
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

/// Tracker: önce `tracker3 reset`, yoksa dizin süpürme.
pub fn execute_tracker(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !tracker_available() {
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::t(&ctx.lang, "no Tracker index found"),
            None,
            0,
        ));
        return;
    }
    let size = tracker_size();
    if ctx.dry_run {
        let method = if have("tracker3") {
            "tracker3 reset --filesystem"
        } else if have("tracker") {
            "tracker reset -e"
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
        if let Some(out) = command_output("tracker3", &["status"]) {
            for line in out.lines().take(6) {
                report.push(Entry::new(
                    EntryKind::Command,
                    cleaner,
                    option,
                    crate::i18n::et(&ctx.lang, "tracker: {}", &[&line.to_string()]),
                    None,
                    0,
                ));
            }
        }
        return;
    }
    if have("tracker3") {
        match crate::fsutil::run_command("tracker3", &["reset", "--filesystem"], true) {
            Ok((0, _, _)) => {
                report.push(Entry::new(
                    EntryKind::Command,
                    cleaner,
                    option,
                    crate::i18n::et(
                        &ctx.lang,
                        "Tracker index reset (~{})",
                        &[&crate::deep::safety::human(size)],
                    ),
                    None,
                    size,
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
                        &[&"tracker3 reset".to_string(), &code.to_string(), &stderr],
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
    if let Some(h) = home() {
        for leaf in ["tracker", "tracker3"] {
            crate::deep::clean_dir_contents(
                &h.join(".cache").join(leaf),
                ctx,
                cleaner,
                option,
                report,
                8,
            );
            crate::deep::clean_dir_contents(
                &h.join(".local").join("share").join(leaf),
                ctx,
                cleaner,
                option,
                report,
                8,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::const_is_empty)]
    fn leaves_are_relative_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for leaf in APP_LEAVES.iter().chain(LOCAL_LEAVES.iter()) {
            assert!(!leaf.starts_with('/'), "mutlak yol yasak: {leaf}");
            assert!(!leaf.contains(".."), "üst dizin yasak: {leaf}");
            assert!(seen.insert(*leaf), "yinelenen yaprak: {leaf}");
        }
        assert!(!APP_LEAVES.is_empty());
    }

    #[test]
    fn extensions_never_targeted() {
        for leaf in APP_LEAVES.iter().chain(LOCAL_LEAVES.iter()) {
            assert!(
                !leaf.contains("extensions"),
                "eklenti dizini hedeflenemez: {leaf}"
            );
        }
    }
}
