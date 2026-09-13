//! Helix düzenleyici önbellek ve geçici durum kalıntıları.
//!
//! Hedefler:
//! * `~/.cache/helix` — indirilen/ compile-out paving önbelleği,
//! * `~/.local/state/helix` — history, yank, otomatik kayıtlar (kullanıcı dokunamaz).
//!
//! Bu modül çalışan helixten haberdar olmaz; insanlar Cleanup yaparken dosya
//! kilidi olasılığı düşüktür. `helix` çalışıyorsa temizlik yapmaz.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

fn cache_dir() -> Option<PathBuf> {
    crate::platform::home_dir().map(|h| h.join(".cache").join("helix"))
}

fn state_dir() -> Option<PathBuf> {
    crate::platform::home_dir().map(|h| h.join(".local").join("state").join("helix"))
}

fn is_running() -> bool {
    if have("hx") {
        return !command_output("pidof", &["hx"]).map(|o| o.trim().is_empty()).unwrap_or(true);
    }
    false
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if is_running() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "helix is running"));
        return;
    }

    if ctx.dry_run {
        for dir in [cache_dir(), state_dir()].iter().flatten() {
            let (bytes, files) = crate::deep::dir_stats(dir);
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "helix dir to clean: {} ({} files)",
                    &[&dir.display().to_string(), &files.to_string()],
                ),
                Some(dir.as_path()),
                bytes,
            ));
        }
        return;
    }

    for dir in [cache_dir(), state_dir()].iter().flatten() {
        crate::deep::guarded_delete(dir, ctx, cleaner, option, report);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_absolute() {
        for dir in [cache_dir(), state_dir()].iter().flatten() {
            assert!(dir.is_absolute(), "helix path must be absolute: {dir:?}");
        }
    }
}
