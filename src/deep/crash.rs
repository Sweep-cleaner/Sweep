//! Çökme dökümleri: apport (`/var/crash`), ABRT, systemd-coredump.
//!
//! Dökümler hata ayıklama dışında işe yaramaz ve GB'larca büyüyebilir.
//! Kök dizinlerin kendisi korunur, yalnızca döküm dosyaları hedeflenir.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

/// Kapsam: dağıtım dökümleri ya da coredump deposu.
#[derive(Debug, Clone, Copy)]
pub enum Scope {
    Dumps,
    Coredumps,
}

/// Apport + ABRT döküm dosyaları.
pub fn dump_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for dir in ["/var/crash", "/var/spool/abrt", "/var/tmp/abrt"] {
        let root = PathBuf::from(dir);
        if !root.is_dir() {
            continue;
        }
        let options = crate::fsutil::walk::ScanOptions::files().with_max_depth(6);
        out.extend(crate::fsutil::walk::scan_paths(&root, &options));
    }
    if let Some(home) = crate::platform::home_dir() {
        let user = home.join(".cache").join("abrt");
        if user.is_dir() {
            let options = crate::fsutil::walk::ScanOptions::files().with_max_depth(4);
            out.extend(crate::fsutil::walk::scan_paths(&user, &options));
        }
    }
    out
}

/// systemd-coredump deposu.
pub fn coredump_dir() -> Option<PathBuf> {
    let p = PathBuf::from("/var/lib/systemd/coredump");
    p.is_dir().then_some(p)
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report, scope: Scope) {
    let targets: Vec<PathBuf> = match scope {
        Scope::Dumps => dump_files(),
        Scope::Coredumps => coredump_dir().into_iter().collect(),
    };

    if ctx.dry_run {
        if targets.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no crash dumps"),
                None,
                0,
            ));
            return;
        }
        if matches!(scope, Scope::Coredumps) {
            if let Some(dir) = targets.first() {
                report.push(Entry::new(
                    EntryKind::Delete,
                    cleaner,
                    option,
                    crate::i18n::et(
                        &ctx.lang,
                        "coredump store {}",
                        &[&dir.display().to_string()],
                    ),
                    Some(dir),
                    crate::fsutil::size::dir_size(dir),
                ));
            }
            return;
        }
        for path in &targets {
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "crash dump {}", &[&path.display().to_string()]),
                Some(path),
                crate::fsutil::size::size_of_or_zero(path),
            ));
        }
        return;
    }

    if !crate::deep::safety::is_root() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "requires root"));
        return;
    }
    match scope {
        Scope::Dumps => {
            for path in &targets {
                crate::deep::guarded_delete(path, ctx, cleaner, option, report);
            }
        }
        Scope::Coredumps => {
            if let Some(dir) = coredump_dir() {
                crate::deep::clean_dir_contents(&dir, ctx, cleaner, option, report, 4);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_when_absent() {
        // Kök yoksa panik değil boş liste: güvenilir davranış.
        let _ = dump_files();
        let _ = coredump_dir();
    }
}
