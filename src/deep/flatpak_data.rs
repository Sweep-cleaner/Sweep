//! Flatpak uygulama verisi önbelleği ve geçici dosyalar.
//!
//! Hedefler (kullanıcı 일에lemeli):
//! * `~/.var/app/*/cache` — her Flatpak uygulamasının sahip olduğu hareketli önbellek,
//! * `~/.var/app/*/tmp` — geçici uygulama dosyaları.
//!
//! `flatpak` çalışıyorsa bu dizinler yok sayılır; yoksa da temizlenebilir.
//! Kullanıcı aplikasyonlarıyla etkileşim yok; sadece önbellek/kopya dizinleri.
//!
//! Bu modül flatpak olmasa bile işlevselliğini korur; yalnızca ~/.var/app varsa
//! o anlama göre temizleme yapar.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

fn var_app_dirs() -> Vec<PathBuf> {
    let home = crate::platform::home_dir().unwrap_or_else(|| PathBuf::from("/home"));
    let base = home.join(".var").join("app");
    if !base.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&base) {
        for entry in entries.flatten() {
            let child = entry.path();
            for sub in ["cache", "tmp"] {
                let p = child.join(sub);
                if p.is_dir() {
                    out.push(p);
                }
            }
        }
    }
    out
}

fn is_flatpak_running() -> bool {
    // Basit süreç kontrolü.
    crate::deep::have("flatpak")
        && crate::deep::command_output("pidof", &["flatpak", "flatpak-system-helper"])
            .map(|o| !o.trim().is_empty())
            .unwrap_or(false)
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if is_flatpak_running() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "flatpak is running"));
        return;
    }

    if ctx.dry_run {
        for dir in &var_app_dirs() {
            let (bytes, files) = crate::deep::dir_stats(dir);
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "flatpak app data to clean: {} ({} files)",
                    &[&dir.display().to_string(), &files.to_string()],
                ),
                Some(dir.as_path()),
                bytes,
            ));
        }
        if var_app_dirs().is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no flatpak app cache dirs found"),
                None,
                0,
            ));
        }
        return;
    }

    for dir in &var_app_dirs() {
        crate::deep::guarded_delete(dir, ctx, cleaner, option, report);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_dirs_are_absolute() {
        for d in var_app_dirs() {
            assert!(d.is_absolute(), "flatpak data dir must be absolute: {d:?}");
        }
    }
}
