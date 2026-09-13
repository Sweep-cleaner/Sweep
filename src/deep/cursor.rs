//! Cursor düzenleyici kalıntıları: VS Code tabanlı, kendi `~/.config/Cursor`
//! ve `~/.cache/Cursor` dizinleri.
//!
//! Hedefler:
//! * `~/.config/Cursor/User/workspaceStorage` — workspace önbellek,
//! * `~/.config/Cursor/logs` — uygulama günlükleri,
//! * `~/.config/Cursor/CachedData` ve `GPUCache` — önbellek dosyaları,
//! * `~/.cache/Cursor` — indirilen/antaretik önbellekler.
//!
//! Uzak sunucu (Remote-SSH / containers) logları ve önbellekleri de temizlenebilir
//! ancak uzaktaki projeler, eklentiler ve yapılandırmalar asla dokunulmaz.
//!
//! `cursor` çalışıyorsa temizlik yapmaz.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

fn config_dir() -> Option<PathBuf> {
    crate::platform::home_dir().map(|h| h.join(".config").join("Cursor"))
}

fn cache_dir() -> Option<PathBuf> {
    crate::platform::home_dir().map(|h| h.join(".cache").join("Cursor"))
}

fn local_share_dir() -> Option<PathBuf> {
    crate::platform::home_dir().map(|h| h.join(".local").join("share").join("Cursor"))
}

fn is_running() -> bool {
    if have("cursor") {
        return !command_output("pidof", &["cursor"]).map(|o| o.trim().is_empty()).unwrap_or(true);
    }
    false
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if is_running() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "cursor is running"));
        return;
    }

    if ctx.dry_run {
        for base in [config_dir(), cache_dir(), local_share_dir()] {
            if let Some(base) = base {
                if base.is_dir() {
                    let (bytes, files) = crate::deep::dir_stats(&base);
                    report.push(Entry::new(
                        EntryKind::Delete,
                        cleaner,
                        option,
                        crate::i18n::et(
                            &ctx.lang,
                            "cursor dir to clean: {} ({} files)",
                            &[&base.display().to_string(), &files.to_string()],
                        ),
                        Some(base.as_path()),
                        bytes,
                    ));
                }
            }
        }
        return;
    }

    for base in [config_dir(), cache_dir(), local_share_dir()] {
        if let Some(base) = base {
            crate::deep::guarded_delete(&base, ctx, cleaner, option, report);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_absolute() {
        for b in [config_dir(), cache_dir(), local_share_dir()] {
            if let Some(p) = b {
                assert!(p.is_absolute(), "cursor path must be absolute: {p:?}");
            }
        }
    }
}
