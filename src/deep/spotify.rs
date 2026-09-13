//! Spotify Linux önbelleği ve günlükleri.
//!
//! Hedefler ( kullanıcı tarafından ssh edilen):
//! * `~/.cache/spotify/Cached*`, `~/.cache/spotify/Browser` — indirimli önbellek
//!   dosyaları (çevrimdışı şarkılar yeniden indirilir, kayıtlar korunur),
//! * `~/.config/spotify/logs/*.log*` — uygulama günlükleri (hesaplar korunur).
//!
//! `spotify` veya `spotify-muzak` çalışıyorsa temizlik yapmaz (çalışan uygulamaya
//! müdahale etmemek için guard). `running.type = "exe"` + `value = "spotify"` ile
//! tanımlanır; temizleyici.toml'da artık `running` bölümü yoksa da çalışma anı
//! bu modül içinde kontrol edilir.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
// `have` is only consulted on the platform branches that guard on the
// Spotify binary being installed; on others the import would be unused.
#[allow(unused_imports)]
use crate::deep::{command_output, have};

fn cache_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        let c = home.join(".cache").join("spotify");
        if c.is_dir() {
            out.push(c);
        }
    }
    out
}

fn log_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        let logs_dir = home.join(".config").join("spotify").join("logs");
        if logs_dir.is_dir() {
            out.push(logs_dir);
        }
    }
    out
}

fn is_running() -> bool {
    // Basit kontrol: pidof/process lookup spotify süreçleri.
    if let Some(out) = command_output("pidof", &["spotify"]) {
        return !out.trim().is_empty();
    }
    if let Some(out) = command_output("pidof", &["spotify-muzak"]) {
        return !out.trim().is_empty();
    }
    false
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if is_running() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "spotify is running"));
        return;
    }

    if ctx.dry_run {
        for dir in &cache_dirs() {
            let (bytes, files) = crate::deep::dir_stats(dir);
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "spotify cache to clean: {} ({} files)",
                    &[&dir.display().to_string(), &files.to_string()],
                ),
                Some(dir.as_path()),
                bytes,
            ));
        }
        for dir in &log_candidates() {
            let (bytes, files) = crate::deep::dir_stats(dir);
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "spotify logs to clean: {} ({} files)",
                    &[&dir.display().to_string(), &files.to_string()],
                ),
                Some(dir.as_path()),
                bytes,
            ));
        }
        return;
    }

    for dir in &cache_dirs() {
        crate::deep::guarded_delete(dir, ctx, cleaner, option, report);
    }
    for dir in &log_candidates() {
        crate::deep::guarded_delete(dir, ctx, cleaner, option, report);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_and_log_paths_are_absolute() {
        for p in cache_dirs().iter().chain(log_candidates().iter()) {
            assert!(p.is_absolute(), "spotify path must be absolute: {p:?}");
        }
    }
}
