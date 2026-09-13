//! LXC/LXD kalıntıları: container logları ve indirilmiş imaj önbelleği.
//!
//! Hedefler (kullanıcı/root ayrımıyla):
//! * sistem (`/var/lib/lxc/*/log` — container log dosyaları),
//! * `/var/lib/images/lxc` (indirilmiş/placeholder imajlar — yeniden indirilebilir),
//! * kullanıcı `~/.local/share/lxc/*/log` (kullanıcı tanımlı containerek logları).
//!
//! Çalışan containerek, konfigürasyon ve container dizinleri kendi kendine bıraktır.
//! `lxc-info`/`lxc-ls` çağrısı yapmaz; yalnızca dizin/yol bazlı güvenli temizlik yapar.
//!
//! `lxc`/`lxd` yoksa bu modül hiçbir şeyi değiştirmez.

use std::path::{Path, PathBuf};

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::have;

fn system_lxc_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for base in [Path::new("/var/lib/lxc"), Path::new("/var/lib/images/lxc")] {
        if base.is_dir() {
            out.push(base.to_path_buf());
        }
    }
    out
}

fn user_lxc_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        let p = home.join(".local").join("share").join("lxc");
        if p.is_dir() {
            out.push(p);
        }
    }
    out
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !have("lxc-info") && !have("lxd") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["lxc/lxd"]),
        );
        return;
    }

    if ctx.dry_run {
        let sys_dirs = system_lxc_dirs();
        for dir in &sys_dirs {
            let (bytes, files) = crate::deep::dir_stats(dir);
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "lxc system dir to clean: {} ({} files)",
                    &[&dir.display().to_string(), &files.to_string()],
                ),
                Some(dir.as_path()),
                bytes,
            ));
        }
        for dir in &user_lxc_dirs() {
            let (bytes, files) = crate::deep::dir_stats(dir);
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "lxc user dir to clean: {} ({} files)",
                    &[&dir.display().to_string(), &files.to_string()],
                ),
                Some(dir.as_path()),
                bytes,
            ));
        }
        return;
    }

    // Sistem klasörleri root gerektirir.
    for dir in system_lxc_dirs() {
        if !crate::deep::safety::is_root() {
            report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "requires root"));
            return;
        }
        crate::deep::guarded_delete(&dir, ctx, cleaner, option, report);
    }

    for dir in user_lxc_dirs() {
        crate::deep::guarded_delete(&dir, ctx, cleaner, option, report);
    }
}

fn system_lxc_log_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for base in system_lxc_dirs() {
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let child = entry.path().join("log");
                if child.is_dir() {
                    out.push(child);
                } else if child.is_file() {
                    out.push(child);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_lxc_dirs_are_absolute() {
        for p in system_lxc_dirs() {
            assert!(p.is_absolute(), "system lxc dir must be absolute: {p:?}");
        }
    }
}
